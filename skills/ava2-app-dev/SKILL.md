---
name: ava2-app-dev
description: 为运行中的 ava2 平台（AppHost 架构，一切皆 App）编写、安装与调试 App。当 agent 被要求新建或修改 ava2 App、给 App 加视图或能力贡献、安装或卸载 App、或排查 app_* 错误码时使用。本技能自包含，不依赖 ava2 源码仓库。
license: MIT
---

# 为 ava2 开发 App

本技能面向运行在 **ava2 平台上**的 agent：你的用户正在使用一个已启动的 ava2 实例，你要为它编写并安装 App。本技能自包含——正文给出契约不变量与工作流，细节规范随技能分发在 `references/`（见 §0.1），**不依赖 ava2 源码仓库**。

工作流：**确认实例 → 定 id 与目录 → 写 manifest → 写 setup(ctx) → REST 安装 → 观察调试 → bump version 迭代**。

## 0. 运行环境与参考资料（先确认再动手）

### 0.1 随本技能分发的参考资料（`references/`）

按优先级（与正文冲突时以正文契约不变量为准，细节以参考资料为准）：

| 文件 | 内容 | 何时读 |
| --- | --- | --- |
| `app-contract.json` | 机器可读契约 SPEC：manifest 字段 / 枚举 / slot 目录 / ctx 能力面 / AgentChannel 方法 / 总线事件 / 安装端点 | 校验字段与枚举；仓库 `npm run check:contracts` 以 `contracts/app-contract.json` 为唯一基准（同源） |
| `standards.md` | 完整开发规范：契约权威链 / 流程 / **§2.4 两种形态完整骨架示例（新 App 照抄起步）** / setup / slot / ctx / contributes / 样式 / §9 交付自检清单 | 新建 App 前、交付前 |
| `DESIGN.md` | App UI 设计指南：令牌速查 / 组件配方 / 可视化锚点 / 反模式禁令 | 做或改任何 UI 前 |
| `icon-spec.md` | 图标规范（24×24 viewBox path 写法与风格） | 声明 manifest `icon` 或 slot `icon` 前 |
| `agent-channel.md` | AgentChannel 全方法签名与语义 | 用 `ctx.agent` 高频方法之外的能力前 |
| `errors.md` | `app_*` 错误码对照 + 运行期外联越权排查 | 安装失败 / App `failed` / 外联报错时 |

### 0.2 运行环境（先确认再动手）

- 目标是**已运行**的 ava2 实例（桌面端或 web-ui 形态）。动手前先确认其边车 REST 地址：web-ui 直启形态默认 `http://127.0.0.1:9800`，桌面端由壳**动态分配端口**——不确定时向用户确认，或从当前会话上下文读取。
- 就绪检查：`Invoke-RestMethod http://127.0.0.1:<port>/v1/health` 返回 ok。
- 一个 App = 一个产物目录（`manifest.json` + ESM 入口），**可写在任意工作目录**；经 REST 安装后由边车复制进 `<ONION_HOME>/apps/<id>/` 运行，宿主按登记表增量装配。
- App 装的是**安装那一刻的目录副本**：之后修改源码目录对已装 App 无效，改完必须重新安装并 bump `version`。
- 观察效果：安装后宿主增量装配、**无需重启**；UI 变化在 ava2 前端界面直接可见。

## 1. 写 manifest

在 App 目录创建 `manifest.json`：

```json
{
  "id": "my-app",
  "name": "我的应用",
  "version": "0.1.0",
  "hostApi": 2,
  "description": "一句话简介（应用市场网格与详情抽屉展示）",
  "author": "开发者/团队展示名",
  "homepage": "https://example.com/my-app",
  "license": "MIT",
  "icon": "M10 2a8 8 0 1 0 0 16 8 8 0 0 0 0-16z",
  "inject": ["slots", "agent"],
  "surfaces": ["surface.main", "nav.sidebar"],
  "entry": "app.js"
}
```

硬性规则：

- `id`：`^[A-Za-z0-9._-]{1,64}$`，kebab-case 推荐；与 App 目录名保持一致（约定，非校验）。
- `version`：semver；**每次改动代码都 bump**（入口 URL 带 `?v=<version>` 做 ESM 缓存指纹，不 bump 可能仍跑旧模块）。
- `hostApi`：写 `2`（当前上限即 2，超出安装被拒 `app_host_api_unsupported`）。
- 入口：顶层 `entry` 或 `contributes.apps[].entry` **至少声明一个**；必须是 App 根目录内的相对路径且文件真实存在。纯能力 App（无 UI）也需一个空 `setup` 入口。
- `inject`：只填要用的能力面（`slots` / `events` / `agent` / `storage` / `net`），未声明即访问会响亮报错。
- 联网 App：`inject` 含 `"net"`，并声明 `net: { "allow": ["api.example.com"] }` 裸域名白名单（缺省 = 全部 https 公网，声明即收窄；越权宿主直接抛错）。
- `surfaces`：只填要注入的 slot key（见 §3），越权注入加载期报错。
- 多页面：用 `contributes.apps: [{ id, entry, inject?, surfaces? }]` 逐页声明，页 `id` 在 App 内唯一；此时顶层 `entry` 等价于首页入口。
- 展示字段：`description` / `author` / `homepage` / `license` / `icon` 均可选、缺省不渲染。`icon` 是应用形象的权威声明（24×24 viewBox 的 SVG path 字符串）；未声明时宿主回落遍历 slot 内 icon——纯能力 App 声明 `icon` 后应用宫格也能有图标。
- 能力贡献（`contributes`，与 UI 独立可组合，由边车在每次会话装配期注入，安装/启停后新会话即时生效）：
  - `skills`：`["skills"]`（技能根，每子目录一个 `SKILL.md`）或 `["skills/x/SKILL.md"]`（单文件）。
  - `mcpServers`：内联 `[{ name, command?, url?, args?, env? }]` 或 `{ "path": "mcp.json" }`；`command` 即 stdio、`url` 即 streamable-http，二者其一；命令可用 `${CLAUDE_PLUGIN_ROOT}` 引用 App 内脚本；与既有 server 重名时跳过。
  - `hooks`：内联 `[{ event, matcher?, handler }]` 或 `{ "path": "hooks.json" }`；Claude Code 兼容格式，边车**只执行 `command` 处理器**（stdin 收 JSON 载荷，stdout 输出决策 JSON），`bus` 处理器会被跳过。
  - 所有路径必须落在 App 根目录内（拒绝绝对路径与 `..`），安装期校验存在性，不满足直接拒绝安装。

写完 manifest 先做契约级本地校验（零依赖 Node 脚本，随本技能提供，路径相对本技能目录）：

```powershell
node <技能目录>\scripts\validate-manifest.mjs <app-dir>
```

0 error 才进入下一步；错误码与服务端一致（app_*，对照 `references/errors.md`）。

## 2. 写 setup(ctx)

`setup(ctx)` 是 App 唯一入口 API，**组件拿不到 ctx**——在 setup 里用闭包把需要的回调和 ctx 捕获后传给组件。

裸 ESM 最小完整示例（React 由宿主 import map 供给，直接 import 即可）：

```js
import * as React from "react";

export function setup(ctx) {
  function Main() {
    const [count, setCount] = React.useState(0);
    return <div className="myapp-root"><button onClick={() => setCount(count + 1)}>{count}</button></div>;
  }
  const d1 = ctx.slots.inject("nav.sidebar", { order: 10, viewId: "my-app", title: "我的应用" });
  const d2 = ctx.slots.inject("surface.main", {
    viewId: "my-app", title: "我的应用",
    component: async () => ({ default: Main }),   // 必须惰性工厂
  });
  return { dispose: () => { d1.dispose(); d2.dispose(); } };
}
```

不变量（逐条为验收基线）：

- setup 只登记 slot / 订阅，**不做 DOM 或网络副作用**；`component` 一律是惰性工厂 `async () => ({ default: Comp })`，只返回组件函数、不返回已渲染 element。
- `events.on` 返回的 Disposable 在 React cleanup 中必须写 `() => d.dispose()`——直接 `d.dispose()` 会在每次渲染期提前注销订阅。
- 平台服务一律经 `ctx`（Agent 通道 `ctx.agent`、总线 `ctx.events`、KV `ctx.storage`）；App **不直连 WS、不 fetch 边车以外的地址、不使用宿主特权**。
- 配置完全由 App 自管（宿主零策略）：存储、校验、回显自理；需要「设置」入口时在 setup 期 `ctx.registerSettingsPanel(() => import("./settings.js"))` 登记面板惰性工厂，宿主只提供入口按钮 + 惰性挂载 + 错误隔离。
- App 卸载 / 禁用时宿主自动撤回 slot 与订阅；setup 返回的 `dispose` 只用于释放自有资源。

## 3. slot 目录（六类）

| slot key | 形态 | viewId 语义 | 界面落点 |
| --- | --- | --- | --- |
| `surface.main` | 认领型 | 路由键，**全局唯一**（冲突加载期报错） | 主区 tab 舞台（可脱离 tab 悬浮） |
| `surface.canvas` | 认领型 | 路由键，**全局唯一** | 画布第二舞台 tab |
| `nav.sidebar` | 引用型 | 指向本 App 的主区视图 | 左侧导航 |
| `panel.sidebar` | 引用型 | 跟随 `viewId` 对应的主区视图 | 侧栏下方面板区 |
| `panel.canvas` | 引用型 | 跟随 `viewId` 对应的画布视图 | 画布舞台下方面板区 |
| `user.menu` | 引用型 | 无（菜单项非视图） | 侧栏 user 栏上浮菜单的 App 区 |

spec 形态：

- `surface.*`：`{ viewId, title, component }`（canvas 另有 `icon?` / `description?`）。
- `nav.sidebar`：`{ order, viewId, title, icon? }`，按 `order` 升序。
- `panel.*`：`{ order, viewId, title, component }`，按 `viewId` 过滤 + `order` 排序。
- `user.menu`：`{ id, title, icon?, order?, action? }`，按 App 名分组、组内 `order`（缺省 100）排序；`action` 为纯数据动作（`openView` / `openSettings` / `event`），宿主解释执行。

状态驻留：tab 切换 / 悬浮还原会**卸载**非激活视图；`panel.*` 与对应 `surface.*` 是独立组件实例、不共享 state——跨切换状态进 `ctx.storage`，瞬时协同走自定义总线事件（事件名加 App id 前缀防撞名）。

## 4. ctx 能力面

| 面 | API | 要点 |
| --- | --- | --- |
| `ctx.manifest` | 只读清单 | 含宿主注入的 `baseUrl` 与 `pageId`；App 不得自填 `baseUrl` |
| `ctx.slots.inject(key, spec)` | → Disposable | 见 §3 |
| `ctx.events.on / emit` | → Disposable / void | 见下事件总线 |
| `ctx.agent` | AgentChannel | 平台 Agent 通道；**不直连 WS**；完整方法清单读 `references/agent-channel.md` |
| `ctx.storage.get / set` | async KV | 命名空间 `app:<id>:`，契约统一异步 |
| `ctx.net.fetch` | 统一联网服务 | App 外联**唯一出口**，要点见下 |
| `ctx.registerSettingsPanel(factory)` | void | setup 期登记设置面板惰性工厂 |

高频 AgentChannel 方法：`listSessions` / `getSession` / `createSession(cwd?)` / `send` / `onSessionEvent` / `getPending` / `replyApproval` / `replyPlanReview` / `answerQuestion` / `listModels` / `setSessionModel` / `setApprovalPreset` / `interrupt` / `compact` / `readWorkspaceFile` / `getTodos` / `getGoal` / `setGoal` / `getPlan` / `setPlan`。签名与语义见 `references/agent-channel.md`。

**事件总线**：`on` 收广播事件 + 定向到本 App 的事件；`emit` 的回执类（如 `agent.hookResult`）只回宿主，其余广播。已约定事件：`canvas.open { viewId }`、`canvas.openPath { path }`、`agent.invoke` / `agent.reply`。

**统一联网服务**：`ctx.net.fetch({ url, method?, headers?, body?, encoding? }) → { status, contentType, body }`。要点：

- App 内**禁止自行 `fetch` 第三方站点**（浏览器 CORS 白名单不可控，必然失败），外联一律走 `ctx.net.fetch`；宿主侧白名单来自 manifest `net.allow`。
- 上游状态码（404/405 等）**原样透传**，按上游语义处理，勿当作宿主错误。
- 服务端强制：仅 https 公网、请求头白名单、响应 ≤4MiB、超时 15s。

## 5. 产物形态

按 **UI 复杂度与产品要求**决策，不是按「能不能省掉构建」：

| 形态 | 适用场景 | 判断信号 |
| --- | --- | --- |
| A · 裸 ESM（`manifest.json` + `app.js`，装源码目录） | 纯能力 App（hooks / mcp / skills 贡献，无 UI 或 UI 只是配角）；极简小工具（单视图、静态布局，如时钟、开关面板） | UI 总量 ≤ ~80 行 JSX 等价物、**无列表渲染、无视图切换、无动效** |
| B · 构建型（React + esbuild：`src/main.tsx` + `build.mjs` → 装 `dist/`） | **一切面向用户的产品级 UI**：数据密集（榜单 / 卡片 / 图表 / 表格）、多 tab 或多面板联动、需要动效与过渡、组件复用与状态管理 | 任一信号即选 B：列表渲染、条件视图切换、UI 超过 ~100 行 |

**默认规则：面向用户的展示型 App 一律选形态 B。** 用形态 A 实现榜单 / 卡片 / 动效必然产出朴素平铺的列表，评审不过。

**形态 A 写法**：App 目录直接放 `manifest.json` + `app.js`，无需任何构建步骤；react 由宿主 import map 供给（`import * as React from "react"` 可直接用）。模板见 §2。

**形态 B 写法**：用 esbuild 单包产出 `dist/main.js`，**安装源改为 `dist/`**：

```js
// build.mjs 关键不变式：react 系一律 external，运行时由宿主 import map 供给（单实例 React）
await esbuild.build({
  entryPoints: ["src/main.tsx"],
  bundle: true, format: "esm", target: "es2022", jsx: "automatic",
  outfile: "dist/main.js",
  external: ["react", "react-dom", "react/jsx-runtime", "react/jsx-dev-runtime", "react-dom/client"],
});
```

## 6. 代码结构规范（按 slot 拆分）

多于一个视图的 App **必须按 slot 拆分文件**，不要把全部组件堆进单个 `main.tsx`。标准目录（每个 App 完全自包含——不依赖其他 App、不与任何其他 App 共享代码，重复的辅助函数宁可各写一份；唯一的对外依赖是 SDK 类型包）：

```
apps/<id>/src/
├── main.tsx      # setup() 入口 + surface.main 主视图（唯一导出 setup 的文件）
├── canvas.tsx    # surface.canvas 画布视图（有认领才建）
├── panels.tsx    # panel.* / user.menu 小视图（可合并一个文件）
├── lib.ts        # 纯逻辑：文件枚举 / 解析 / 统计（不 import React、不碰 DOM）
└── styles.ts     # 本 App CSS 字符串 + <Styles/> 注入组件
```

规则：

- 单文件目标 <500 行，超过约 800 必须拆。
- UI 与逻辑分层：组件只做渲染与调度；`readWorkspaceFile` 之后的解析、统计、聚合全部进 `lib.ts` 纯函数，便于复用与排查。
- 跨 slot 数据共享：setup 期单例 props 下传，或 `ctx.events` 广播 + `ctx.storage` 持久化兜底——**订阅方组件晚挂载时必须能从 storage 恢复**，不能只依赖事件。
- 交互基线：文件/工作区枚举处提供「刷新」按钮；出错区提供「重试」入口；上次选择（工作区/文件）用 `ctx.storage` 记忆并在下次打开恢复。
- `manifest.json` 与 `package.json` 的 `version` 同步 bump。

### 6.1 列表/市场类视图 UI 范式

条目集合页（技能广场、模型列表、文件库等）遵循统一范式（实测沉淀）：

- **初始即加载**：组件挂载 `useEffect` 立即以默认参数拉取（搜索类上游对**空关键词返回全量/热门列表**），禁止把数据初值留空等用户先搜索——进入即空白是最常见的体验 bug。
- **卡片网格**：`display: "grid"` + `repeat(auto-fill, minmax(240px, 1fr))` + `gap: var(--ava-space-4)`，不要 flex 横排列表行；卡片是大圆角（`--ava-radius-panel`）软卡片，结构为：图标/头像 → 徽标行 → 标题 → 两行截断摘要（`WebkitLineClamp: 2` + `WebkitBoxOrient: "vertical"`）→ 元信息行 → footer 操作区（border-top 分隔）。
- **加载/空态**：加载中显示网格骨架占位卡（pulse 动画经组件根部注入 `<style>` 的 keyframes，inline style 无法定义）；空结果给虚线框 + 图标 + 文案，不要裸文字。
- **操作按钮 hover 显形**：`opacity: hover ? 1 : 0` + `visibility: hover ? "visible" : "hidden"` 过渡——`visibility` 保持布局占位（footer 不跳动）且隐藏态不可误点；状态徽标（启用中/已安装等）始终可见。每卡片局部 `useState` 存 hover，避免整表重渲染。
- **按钮必须 `whiteSpace: "nowrap"`**：footer 横向拥挤时按钮文字会折行裁切（如「安装」只露出「安」）。
- **强调色唯一令牌是 `--ava-state-business-primary`**（亮 #4176e6 / 暗 #679efe）：token 契约里**不存在** `--ava-accent`，写它会静默落到 fallback；警示/星标琥珀用 `--ava-state-warning`，成功绿 `--ava-state-success`，危险红 `--ava-state-danger`。令牌源 `src/styles/tokens.css` 亮暗两套，消费端只写 `var(--ava-*)`。

### 6.2 设计与美学硬要求

「能用」不算达标，交付物必须达到产品级观感（数值、组件配方与反模式全文以 `references/DESIGN.md` 为准，图标写法见 `references/icon-spec.md`）：

1. **三段式结构**：标题区（主标题 + 一句副标题）→ 工具栏（分段控件 / 搜索 / 状态信息）→ 内容区 → 页脚元信息；间距走 `--ava-space-*` 节奏（2/4/8/10/16/24），禁止均匀堆砌。
2. **视觉锚点**：榜单/统计类必须有排名徽标（前三名品牌色）、量值可视化（mini bar / 中文量级 / `tabular-nums`）、语义 pill（新/热/沸色彩语义化）。
3. **微交互**：hover/active 120–200ms ease；hover 浮起用 `translateY(-2px) + var(--ava-shadow-panel)`；加载用骨架屏脉动（**禁 spinner**）；空态/错误态给文案 + 行动按钮。
4. **反面清单（评审即拒）**：等宽纯文本平铺无焦点、单一字号贯穿、列表无徽章无操作反馈、无 hover 反馈、硬编码色值字号、用 spinner、进入即空白等用户搜索。

## 7. 安装与验证（随本技能提供的脚本）

优先用封装脚本（自动规避 BOM 坑，路径相对本技能目录）：

```powershell
# 裸 ESM 装源码目录；构建型装 dist/
powershell -NoProfile -ExecutionPolicy Bypass -File <技能目录>\scripts\install-app.ps1 -Action install -Path C:\path\to\my-app -Port <port> -Overwrite

powershell -NoProfile -ExecutionPolicy Bypass -File <技能目录>\scripts\install-app.ps1 -Action list    -Port <port>
powershell -NoProfile -ExecutionPolicy Bypass -File <技能目录>\scripts\install-app.ps1 -Action disable -Id my-app -Port <port>
powershell -NoProfile -ExecutionPolicy Bypass -File <技能目录>\scripts\install-app.ps1 -Action uninstall -Id my-app -Port <port>
```

等价的裸 REST（脚本不可用时，注意 body 必须 **UTF-8 无 BOM 字节**）：

```powershell
$body = '{"path":"C:\\path\\to\\my-app","overwrite":true}'
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:<port>/v1/apps/install `
  -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body))

Invoke-RestMethod http://127.0.0.1:<port>/v1/apps                     # 已装清单（manifest + enabled）
Invoke-RestMethod -Method Put -Uri http://127.0.0.1:<port>/v1/apps/my-app/enabled `
  -ContentType 'application/json' -Body '{"enabled":false}'          # 启停（持久化）
Invoke-RestMethod -Method Delete -Uri http://127.0.0.1:<port>/v1/apps/my-app   # 卸载（404 幂等）
```

注意：

- 安装前先跑 `node <技能目录>\scripts\validate-manifest.mjs <app-dir>`（见 §1），0 error 再安装。
- 在 ava2 **仓库根**可跑全量契约校验（SPEC 防漂移 + 全部 App manifest）：`npm run check:contracts`（即 `node scripts/check-app-contract.mjs`），0 error 才算交付。
- `path` 指向 App 产物目录（裸 ESM 源码目录 / 构建型 `dist/`），必须是含 `manifest.json` 的目录。
- 安装后宿主增量装配，无需重启；前端界面即可观察导航 / 主区 / 画布是否出现。
- **安装返回错误码或 App 进入 `failed` 时，读 `references/errors.md` 按表排查后重试**；运行期 `failed` 的具体原因展示在 ava2 的 App 管理面板。

## 8. Gotchas（不知必踩坑）

- **改代码必 bump `version`**：入口 URL 按 `?v=<version>` 缓存，不 bump 可能仍跑旧模块。
- **React 系一律 external**（构建型）：宿主经 import map 供单实例 React，打包第二份则 hooks 必崩。
- 安装 body 带 BOM → `app_manifest_invalid`（见 §7）。
- `events.on` 的 Disposable cleanup 必须写 `() => d.dispose()`（见 §2）。
- 认领型 slot（`surface.*`）的 `viewId` 全局唯一，与其他 App 冲突即加载失败；引用型 `panel.*` 的 `viewId` 必须指向已认领的视图，否则面板永不出现。
- setup 里发网络请求 / 操作 DOM 属于副作用违例——数据获取一律放组件 `useEffect`。
- **外联唯一出口是 `ctx.net.fetch`**：App 内自行 `fetch` 第三方站点必因浏览器 CORS 失败；外联返回的 404/405 是上游状态码原样透传，按上游语义处理，勿当作宿主错误。
- 样式限域在自有根类下（如 `.myapp-root`），颜色字号一律消费 CSS 变量令牌，禁止硬编码，亮暗主题都要覆盖。
- 样式类名加 App id 前缀防撞名；自定义总线事件名同样加前缀。
- **PS 5.1 控制台乱码是显示假象**：`Invoke-RestMethod` 回显安装响应里的中文 manifest 常显示为 `æè½ç®¡ç` 之类乱码，那是 GBK 控制台渲染问题，落盘数据可能是好的——验证时用 `curl.exe -o <file>` 存原始响应字节后读文件（或直接看 UI），不要凭控制台回显下结论。
- **验证以浏览器实测为准**：改完 bump version → 重装 → 用浏览器技能打开 AppHost 页面（dev 形态 `http://localhost:1420/?backend=http://127.0.0.1:<port>`）走一遍关键交互并截图，静态检查通过不等于运行正确。

## 9. 迭代交付

每轮修改后：bump `version` → 重新安装（`overwrite: true`）→ 在前端验证 → 逐条核对 §2 不变量与 §8 Gotchas → 过一遍 `references/standards.md` §9 交付自检清单。卸载重装也是有效的复位手段（卸载 404 幂等）。
