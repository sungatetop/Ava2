---
name: app-development
description: 在 ava2（AppHost 架构，一切皆 App）中新建、修改、安装或调试 App。覆盖契约读取顺序、裸 ESM 与 esbuild 构建两种产物形态、manifest 与 setup(ctx) 写法、ctx.net 统一联网服务、contributes 能力贡献（skills/mcpServers/hooks）、安装启停 REST、契约校验与 app_* 错误码排查。当用户要求开发 ava2 App、给 App 加视图或能力贡献、安装/卸载 App、或遇到 app_manifest_invalid / app_entry_not_found 等错误码时使用。
license: MIT
metadata:
  author: Ava Team
  version: "0.3.1"
---

# 开发 ava2 App

ava2 里**一切皆 App**：一个 App = 一个产物目录（`manifest.json` + ESM 入口），安装即由边车复制进 `<ONION_HOME>/apps/<id>/` 并登记，宿主按登记表增量装配。

工作流：**读契约 → 定目录与 id → 选形态 → 写 manifest → 写 setup(ctx) → 安装验证 → 校验交付**。

## 1. 先读契约（勿凭记忆）

ava2 仓库根：App 开发会话所在的工作区根目录（含 `contracts/` / `apps/` / `packages/` / `docs/`）。**下文仓库路径均相对仓库根展开**；必要参考资料已随本技能分发，见下。

按优先级（冲突以靠前者为准）：

1. **契约 SPEC（随本技能分发）：`references/app-contract.json`** —— 机器可读：manifest 字段 / 枚举 / slot 目录 / ctx 能力面 / AgentChannel 方法 / 总线事件 / 安装端点 / 约束。仓库校验脚本以 `contracts/app-contract.json` 为唯一基准。
2. **完整规范（随本技能分发、自包含）：`references/standards.md`** —— 开发全流程 / slot / ctx 能力面（含联网服务）/ contributes / 样式令牌 / 交付自检。
3. **设计基准（随本技能分发）：`references/DESIGN.md`** 与 **图标规范：`references/icon-spec.md`** —— 做 UI 前必读。
4. 仓库 `src/host/types.ts` —— 类型单一来源（`@ava2/app-sdk` 即它，type-only；代码文件，按需读仓库）。
5. **两种形态的完整骨架示例（目录结构 + 可照抄代码）：`references/standards.md` §2.4** —— 形态 A（裸 ESM：manifest + app.js）与形态 B（React 构建型：package/tsconfig/build.mjs/main.tsx），新 App 直接照骨架起步。

## 2. 定目录与 id

```
ava2/apps/<id>/          # 源码位置；<id> 必须与 manifest.id 完全一致
```

`id` 规则：`^[A-Za-z0-9._-]{1,64}$`，kebab-case 推荐（如 `my-app`）。

## 3. 选形态

按 **UI 复杂度与产品要求**决策，不是按「能不能省掉构建」：

| 形态 | 适用场景 | 判断信号 |
| --- | --- | --- |
| A · 裸 ESM（`manifest.json` + `app.js`，装源码目录） | 纯能力 App（hooks / mcp / skills 贡献，无 UI 或 UI 只是配角）；极简小工具（单视图、静态布局、无列表无动效，如时钟、开关面板） | UI 总量 ≤ ~80 行 JSX 等价物、**无列表渲染、无视图切换、无动效** |
| B · 构建型（React + esbuild：`src/main.tsx` + `build.mjs` → 装 `dist/`） | **一切面向用户的产品级 UI**：数据密集（榜单 / 卡片 / 图表 / 表格）、多 tab 或多面板联动、需要动效与过渡、组件复用与状态管理 | 任一信号即选 B：列表渲染、条件视图切换、需要 framer-motion、UI 超过 ~100 行 |

**默认规则：面向用户的展示型 App 一律选形态 B。** 用形态 A 实现榜单 / 卡片 / 动效必然产出朴素平铺的列表，评审不过。

工程细节（build.mjs 完整文件、package.json / tsconfig.json、两种形态的完整骨架代码）：见 `references/standards.md` §2.2–2.4，新 App 直接照 §2.4 骨架起步。

## 4. 写 manifest

最小模板（`<id>` 必须与目录名一致）：

```json
{
  "id": "my-app", "name": "我的应用", "version": "0.1.0", "hostApi": 2,
  "inject": ["slots"], "surfaces": ["surface.main"],
  "contributes": { "apps": [{ "id": "main", "entry": "app.js" }] }
}
```

核心规则：

- `inject` 只填要用的能力面（`slots` / `events` / `agent` / `storage` / `net`），未声明即访问会响亮报错。
- 联网 App：`inject` 含 `"net"`，`net: { "allow": ["api.example.com"] }` 裸域名白名单（缺省 = 全部 https 公网，声明即收窄；越权宿主直接抛错）。
- **改代码必 bump `version`**：入口 URL 带 `?v=<version>` 破 ESM 缓存，不 bump 可能仍跑旧模块。

完整字段说明（入口双形态 / 多页面 contributes / 展示字段 author·homepage·license·icon / 全部硬性规则）：`references/standards.md` §2.3。

能力贡献（`contributes`）另四类由边车在**每次会话装配期**注入，安装 / 启停后新会话即时生效：

- `skills`：`["skills"]`（技能根，每子目录一个 `SKILL.md`）或 `["skills/x/SKILL.md"]`（单文件）。
- `mcpServers`：内联 `[{ name, command?, url?, args?, env? }]` 或 `{ "path": "mcp.json" }`；命令可用 `${CLAUDE_PLUGIN_ROOT}` 引用 App 内脚本（装配期替换为 App 根目录）；与既有 server 重名时跳过。
- `hooks`：内联 `[{ event, matcher?, handler }]` 或 `{ "path": "hooks.json" }`；边车**只执行 `command` 处理器**，`bus` 会被跳过。
- 所有路径必须落在 App 根目录内（拒绝绝对路径与 `..`），安装期校验存在性。

## 5. 写 setup(ctx)

`setup(ctx)` 是 App 唯一入口 API，**组件拿不到 ctx**——在 setup 里用闭包把需要的回调和 ctx 捕获后传给组件。完整代码骨架见 `references/standards.md` §2.4（两种形态）与 §3（不变量全文）。

不变量：

- setup 只登记 slot / 订阅，**不做 DOM 或网络副作用**；组件用惰性工厂 `async () => ({ default: Comp })`。
- 认领型 slot（`surface.*`）的 `viewId` 是路由键，**全局唯一**，冲突加载期报错；引用型（`nav.sidebar` / `panel.*`）的 `viewId` 指向已认领的视图。
- 平台服务一律经 `ctx`（Agent 通道 `ctx.agent`、总线 `ctx.events`、KV `ctx.storage`、联网 `ctx.net`）；App **不直连 WS、不用 Tauri 特权**；外联**一律走 `ctx.net.fetch`**，禁止自行 `fetch` 第三方站点（浏览器 CORS 白名单不可控，必然失败）。
- 配置完全由 App 自管（宿主零策略）：需要「设置」入口时在 setup 期 `ctx.registerSettingsPanel(() => import("./settings.js"))` 登记面板组件，宿主只提供入口按钮 + 惰性挂载 + 错误隔离。

联网服务要点：`ctx.net.fetch({ url, method?, headers?, body?, encoding? }) → { status, contentType, body }`；上游状态码原样透传，按上游语义处理；服务端强制仅 https 公网、请求头白名单、响应 ≤4MiB、超时 15s。细节见 `references/standards.md` §5。

## 6. 设计与美学（产品级 UI 的硬要求）

生成或改造任何 UI 前，**先读随技能分发的 `references/DESIGN.md`（App UI 设计指南：令牌速查 / 组件配方 / 可视化锚点 / 反模式禁令）与 `references/icon-spec.md`**。「能用」不算达标，交付物必须达到产品级观感。硬要求摘要（数值与配方以 DESIGN.md 为准）：

1. **三段式结构**：标题区（主标题 + 一句副标题）→ 工具栏（分段控件 / pill 按钮 / 状态信息）→ 内容区 → 页脚元信息；间距走 `--ava-space-*` 节奏，禁止均匀堆砌。
2. **视觉锚点**：榜单/统计类必须有排名徽标（前三名品牌色）、量值可视化（mini bar / 中文量级 / tabular-nums）、语义 pill（新/热/沸色彩语义化）。
3. **微交互**：hover/active 120–200ms ease；加载用骨架屏脉动（**禁 spinner**）；空态/错误态给文案 + 行动按钮。
4. **反面清单（评审即拒）**：等宽纯文本平铺无焦点、单一字号贯穿、列表无排名无徽章、无 hover 反馈、硬编码色值字号、用 spinner。

## 7. Gotchas（不知必踩坑）

- 安装请求 body 必须是 **UTF-8 无 BOM 字节**：PowerShell `Set-Content` 默认带 BOM 会解析失败，用 `[Text.Encoding]::UTF8.GetBytes($body)`。
- **改代码必 bump `version`**：入口 URL 带 `?v=<version>` 做 ESM 缓存指纹，不 bump 可能仍跑旧模块。
- **React 系一律 external**：宿主经 import map 供单实例 React，打包第二份则 hooks 必崩（build.mjs 的 `external` 数组不可删减）。
- `events.on` 的 Disposable 在 React cleanup 中必须写 `() => d.dispose()`——直接 `d.dispose()` 会在每次渲染期提前注销订阅。
- tab 切换 / 悬浮还原会**卸载**非激活视图；`panel.*` 与对应 `surface.*` 是独立组件实例、不共享 state——跨切换状态进 `ctx.storage`，瞬时协同走自定义总线事件（事件名加 App id 前缀防撞名）。
- 样式限域在自有根类下，颜色字号一律 `var(--ava-*)` / `var(--cc-*)`，禁止硬编码，亮暗主题都要覆盖。
- **外联 404/405 是上游状态码原样透传**，按上游语义处理，勿当作宿主错误；App 内自行 `fetch` 第三方站点必因 CORS 失败，外联唯一出口是 `ctx.net.fetch`。
- App 卸载 / 禁用时宿主自动撤回 slot 与订阅；setup 返回的 `dispose` 只用于释放自有资源。

## 8. 安装与验证（PowerShell）

```powershell
# 安装 REST 的 path 参数需要绝对路径：<repo-root> 替换为 ava2 仓库根的实际绝对路径
# 裸 ESM 装源码目录；构建型装 dist/
$body = '{"path":"<repo-root>/apps/my-app","overwrite":true}'
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:9800/v1/apps/install `
  -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body))

Invoke-RestMethod http://127.0.0.1:9800/v1/apps                     # 已装清单（manifest + enabled）
Invoke-RestMethod -Method Put -Uri http://127.0.0.1:9800/v1/apps/my-app/enabled `
  -ContentType 'application/json' -Body '{"enabled":false}'          # 启停（持久化）
Invoke-RestMethod -Method Delete -Uri http://127.0.0.1:9800/v1/apps/my-app
```

- 端口：web-ui 默认 `9800`（桌面端由壳动态分配，经 `ctx` 之外的 REST 调用需自行确认）。
- 安装后宿主增量装配，**无需重启**；在 `http://localhost:1420/` 观察导航 / 主区 / 画布是否出现。
- **安装返回错误码或 App 进入 `failed` 时，读 `references/errors.md` 按表排查后重试。**

## 9. 交付前自检

```powershell
cd <repo-root>   # ava2 仓库根
npm run check:contracts    # SPEC 防漂移 + 全部 manifest 校验，0 error 才算交付
```

然后逐条过完整自检清单（形态 / 产品级观感 / setup 不变量 / external / 令牌 / ctx / viewId / contributes / version / 冒烟）：`references/standards.md` §9。**0 error + 清单全勾才算交付。**
