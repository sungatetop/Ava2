---
title: App 开发指南
description: 面向三方 App 开发者的 ava2 AppHost 接入指南：清单字段、slot 体系、ctx 能力面、Agent 通道、总线事件、安装与调试全流程。
keywords: AppHost, App 开发, manifest, slot, AgentChannel, 事件总线, esbuild, 安装
author: AI 助手
date: 2026-11-08
updated: 2026-11-09
status: done
category: references
---

# App 开发指南

ava2 的 App 是一个**预构建产物目录**（含 `manifest.json` + ESM 入口），安装后由边车复制进 `<ONION_HOME>/apps/<id>/` 并登记，宿主按登记表装配。壳与 App 走同一契约：导航、主区视图、画布视图、侧栏/画布面板全部来自 App 的 slot 贡献。

参考实现（广义 App = app（UI/UX）+ skills + hooks + mcpServers 的任意组合，按类型选读）：

- 纯 UI：`apps/hello`（裸 ESM，契约冒烟）、`apps/chatapp`（构建型，React 完整对话 App）、`apps/sessionboard`（多 slot 组合示例）
- 纯技能：`apps/app-devkit`（`contributes.skills` 分发 `develop-ava2-app` 技能）
- 纯 MCP：`apps/mcp-echo`（stdio MCP server，零依赖 Node：`mcp__echo__echo` / `mcp__echo__word_count`）
- 纯 hooks：`apps/hooks-guard`（Claude Code 格式 `hooks.json`：UserPromptSubmit 注入 + PostToolUse 观测）
- 全组合：`apps/all-in-one`（UI 页 + 技能 + hooks + `toolbox` MCP server 四类齐备）

**契约单一来源**（改契约先改这里，再同步实现）：

- 机器可读 SPEC：`contracts/app-contract.json`（字段 / 枚举 / slot / ctx / AgentChannel / 总线事件 / 安装端点 / 约束）。
- 类型权威：`src/host/types.ts`（前端经 `@ava-ai/app-sdk` 包引用，type-only；包 d.ts 由 `npm run build:sdk` 从本文件投影，已发布 npm）。
- 一致性校验：`npm run check:contracts`（`scripts/check-app-contract.mjs`）——SPEC ↔ `types.ts` / `manifest.ts` / `rest/apps.rs` 防漂移，并校验 `apps/*/manifest.json` 与 `dist/manifest.json`。
- Agent 可执行版契约：`apps/app-devkit`（纯能力 App，`contributes.skills` 分发 `develop-ava2-app` 技能；装它即让 agent 按契约用 TS 写 App）。

## 1. 最小 App（裸 ESM，无构建）

```
my-app/
├── manifest.json
└── app.js
```

```json
{
  "id": "my-app",
  "name": "我的应用",
  "version": "0.1.0",
  "entry": "app.js",
  "hostApi": 2,
  "inject": ["slots", "events", "agent"],
  "surfaces": ["surface.main", "nav.sidebar"]
}
```

```js
// app.js：无构建环境用 h() 替代 JSX（或走 §3 的 esbuild 构建型）
import { h, render } from "preact"; // 或任意能产 DOM 的方式；构建型可用 react

function Main() {
  return h("div", null, "你好，AppHost");
}

export function setup(ctx) {
  // ① 登记 slot（返回 Disposable，App 卸载时自动撤回）
  const disposers = [
    ctx.slots.inject("nav.sidebar", { order: 10, viewId: "my-app", title: "我的应用" }),
    ctx.slots.inject("surface.main", {
      viewId: "my-app",
      title: "我的应用",
      component: async () => ({ default: Main }), // 惰性工厂：首次渲染才 import
    }),
  ];
  // ② 可选：返回 cleanup（或把 Disposable 都推进数组）
  return { dispose: () => disposers.forEach((d) => d.dispose()) };
}
```

## 2. manifest 字段

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `id` | 是 | 全局唯一标识；安装目录名与登记键 |
| `name` / `version` | 是 | 展示名 / 版本（版本同时用作 ESM URL 缓存指纹 `?v=`） |
| `description` | 否 | 一句话简介：应用市场网格与详情抽屉展示；缺省隐藏简介行 |
| `author` | 否 | 开发者/团队展示名：详情面板「开发者：xxx」展示；缺省不渲染 |
| `homepage` | 否 | 主页/仓库 URL：详情面板可点击链接（仅 `http(s)://` 前缀渲染为链接，其余按纯文本防注入）；缺省不渲染 |
| `license` | 否 | 开源协议：推荐 SPDX 标识符（如 `MIT` / `Apache-2.0`），详情面板「协议：xxx」展示；缺省不渲染 |
| `icon` | 否 | 应用展示图标（AppIcon：24×24 SVG path 字符串或 `{ d, fill }`，约束见 `icon-spec.md`）：应用形象的**权威声明**，安装期静态可得（纯能力 App 也能有图标）；未声明时宿主回落遍历该 App 的 slot 内 icon |
| `entry` | 是* | 顶层 ESM 入口；声明了 `contributes.apps` 时可省略（每页有自己的 entry）。**两者至少一个**——安装器要求每个 App 有可加载入口（纯能力 App 也需一个空 `setup` 入口，见 `apps/app-devkit/app.js`） |
| `hostApi` | 是 | 宿主契约版本：0 = 全量注入（旧）；1 = inject 显式化；2 = 含 contributes。新 App 一律写 `2`（上限即 `HOST_API_SUPPORTED`，超出安装被拒） |
| `inject` | hostApi≥1 | 声明要用的能力面：`slots` / `events` / `agent` / `storage` / `net`；未声明的面访问即响亮报错 |
| `net` | 否 | 联网服务授权（`inject` 含 `"net"` 时生效）：`{ "allow": ["api.example.com"] }`，裸域名数组（host 等于域或为域子域即放行）；**缺省 = 全部 https 公网**，声明即收窄；越权宿主直接抛错 |
| `surfaces` | 建议 | 声明可注入的 slot key；越权 key 加载期报错 |
| `kind` | 否 | `preset`（平台预置，缺省）/ `demo`（演示，画布启动器中排在预置之后） |
| `contributes` | hostApi≥2 | 能力贡献声明：`skills` / `mcpServers` / `agents` / `hooks` / `apps`（UI 多页）。`apps` 由前端宿主装配，`skills` / `mcpServers` / `hooks` 由边车在会话装配期装配（见 2.1）；`agents` 暂未装配 |

### 2.1 能力贡献（contributes）

已启用（`state.json` 中 `enabled: true`）的 App，其 `contributes` 会在**每次会话装配时**被边车重新扫描并注入，安装 / 启停后新会话即时生效。

所有路径都必须落在 App 根目录内（拒绝绝对路径与 `..`），且安装期校验存在性——不满足直接拒绝安装（`app_contributes_invalid`）。

```json
{
  "contributes": {
    "skills": ["skills", "skills/extra/SKILL.md"],
    "mcpServers": [
      { "name": "my-mcp", "command": "node", "args": ["./server.js"] },
      { "name": "remote-mcp", "url": "https://example.com/mcp" }
    ],
    "hooks": [
      {
        "event": "PreToolUse",
        "matcher": "Bash",
        "handler": { "type": "command", "command": "node ${CLAUDE_PLUGIN_ROOT}/hook.js" }
      }
    ]
  }
}
```

- **skills**：`["目录"]` 或 `["xxx/SKILL.md"]`（也可写 `{ "path": "..." }`）。目录按技能根扫描（每个子目录一个 `SKILL.md`），单文件只放行声明的那一个 `.md`；注册为独立 provider（名 `app:<id>:<序号>`），与全局 `<data_home>/skills` 并存。
- **mcpServers**：内联数组（`name` + `command`（stdio）或 `url`（streamable-http），可选 `args` / `env` / `cwd` / `headers` / `toolCallTimeoutMs`），或 `{ "path": "mcp.json" }` 指向与 `<data_home>/mcp.json` 同格式的配置文件。命令支持 `${CLAUDE_PLUGIN_ROOT}`（装配期替换为 App 根目录），如 `{ "command": "node", "args": ["${CLAUDE_PLUGIN_ROOT}/server.mjs"] }`。与既有 server 重名时跳过（`mcp__<server>__<tool>` 不重复注册）。
- **hooks**：内联 `[{ event, matcher?, handler }]`，或 `{ "path": "hooks.json" }` 指向 Claude Code 兼容配置（`{ "hooks": { "PreToolUse": [...] } }`）。多个 App 的配置按事件合并；`${CLAUDE_PLUGIN_ROOT}` 替换为各自 App 根目录、`${CLAUDE_PROJECT_DIR}` 替换为会话工作目录。hook 脚本从 stdin 收 JSON 载荷、stdout 输出 Claude Code 决策 JSON（`hookSpecificOutput.additionalContext` 注入模型输入——多钩子产出全部拼接；`decision:"block"`+`reason` 拦截）；多个钩子都输出时按最严决策生效。边车只执行 `command` 处理器，`bus`（App 前端回调）会在装配期跳过并记日志。

## 3. 形态二：构建型（React + esbuild）

需要 JSX / npm 依赖时用构建型（见 `apps/chatapp`）：

```
my-app/
├── package.json        # devDependencies: esbuild + react
├── build.mjs           # 见下
├── manifest.json       # entry: "main.js"，装 dist/
└── src/main.tsx
```

```js
// build.mjs 关键不变式：react 系一律 external，运行时由宿主 import map 供给（单实例 React）
await esbuild.build({
  entryPoints: ["src/main.tsx"],
  bundle: true,
  format: "esm",
  target: "es2022",
  jsx: "automatic",
  outfile: "dist/main.js",
  external: ["react", "react-dom", "react/jsx-runtime", "react/jsx-dev-runtime", "react-dom/client"],
});
```

构建后**安装源目录是 `dist/`**（`main.js + manifest.json`）。App 内自行打包的 CSS 建议收敛为样式字符串、由组件 `<style>` 注入并限域在自有根类下（如 chatapp 的 `.cc-root`），避免污染壳与其他 App。

## 4. slot 体系（五类）

| slot key | 形态 | viewId 语义 | 壳中的落点 |
| --- | --- | --- | --- |
| `surface.main` | 认领型 | 路由键，**全局唯一**（冲突加载期报错） | 主区 tab 舞台；亦可脱离 tab 悬浮（浮层舞台，壳层窗框机制，App 无感知） |
| `surface.canvas` | 认领型 | 路由键，**全局唯一** | 画布第二舞台 tab |
| `nav.sidebar` | 引用型 | 指向本 App 的主区视图 | 左侧导航（32px 行，图标 18px） |
| `panel.sidebar` | 引用型 | 跟随 `viewId` 对应的主区视图 | 侧栏下方面板区 |
| `panel.canvas` | 引用型 | 跟随 `viewId` 对应的画布视图 | 画布舞台下方面板区 |

规范：

- `surface.*` 的 spec：`{ viewId, title, component }`（canvas 另有 `icon?`（SVG path d）、`description?`）；`component` 一律是 `async () => ({ default: Comp })` 惰性工厂——**setup 只登记、不做副作用**。
- `nav.sidebar` 的 spec：`{ order, viewId, title, icon? }`，`order` 升序排列。
- `panel.*` 的 spec：`{ order, viewId, title, component }`，按 `viewId` 过滤 + `order` 排序。
- 同一 App 可对同一 viewId 注入多次（如多面板），最后一个撤回才释放认领。
- **视图状态驻留范围**：舞台折叠/展开不卸载组件（状态驻留），但 **tab 间切换会卸载**非激活视图——需要跨切换保留的状态进 `ctx.storage` 或可由 snapshot/REST 重建（chatapp / sessionboard 同此模式）。
- **悬浮（浮层舞台）**：用户经 tab 悬浮按钮可把主区视图悬浮于主界面之上；悬浮与还原（回 tab）是一次卸载/重挂，组件 state 同样不驻留。App 无需适配，也不可在悬浮窗内假设宿主窗框存在——弹窗边界 = 舞台边界在浮层下依然成立（浮层 body 即舞台区域）。

## 5. ctx 能力面（setup(ctx) 的唯一 API）

组件**拿不到 ctx**——在 setup 里把需要的回调用闭包捕获后传给组件。

| 面 | API | 说明 |
| --- | --- | --- |
| `ctx.manifest` | 只读清单 | 含 `baseUrl`（宿主注入的产物根 URL）与 `pageId` |
| `ctx.slots.inject(key, spec)` | → Disposable | slot 注入（§4） |
| `ctx.events.on/emit` | → Disposable / void | 平台总线（§7）；App 卸载时订阅自动排空 |
| `ctx.agent` | AgentChannel | 平台 Agent 通道（§6），**App 不直连 WS** |
| `ctx.storage.get/set` | async KV | localStorage 命名空间 `app:<id>:`，契约统一异步 |
| `ctx.net.fetch(input)` | async 联网 | 统一联网服务：`{ url, method?("GET"\|"POST"), headers?, body?, encoding?("text"\|"dataUrl") }` → `{ status, contentType, body }`。须 `inject` 声明 `"net"`，域受 `manifest.net.allow` 约束（缺省 = 全部 https 公网）；越权宿主直接抛错。服务端安全底线：仅 https 公网、重定向逐跳复检、请求头白名单、响应 ≤4MiB、超时 15s；上游状态码原样透传。App 内禁止自行 `fetch` 第三方站点（CORS 必失败） |
| `ctx.settings.get/all` | 只读快照 | `manifest.config` 校验后的配置值；未声明 `settings` 时访问即报错 |

## 6. Agent 通道（ctx.agent）

对 onion-server REST + WS 的收敛封装（每会话一条 WS 多路复用，引用归零即关闭）：

- **会话**：`listSessions()`（**返回顺序未定义**，勿依赖——`createdAtMs` 为可选字段，App 需展示完整列表或自行容忍截断；列表项不含 `model` / `approvalPreset` 读面，需走 `getSession(id)` 详情）/ `getSession(id)`（详情含 `workingDir` / `root` / `model` / `approvalPreset`）/ `createSession(cwd?)`（cwd 即工作区切换：权限根在会话创建期固定，跨工作区 = 新会话）/ `deleteSession(id)`（404 幂等）/ `setSessionModel(id, model)`（运行期切模型，未知模型 400）/ `setApprovalPreset(id, preset)`（运行期切审批，preset ∈ `workspace-write` / `auto` / `danger-full-access`，非法值 400）——两者均返回更新后的 SessionInfo
- **消息**：`send(id, text)`（异步受理，进度走事件流）/ `getMessages(id)` / `interrupt(id)` / `compact(id)`
- **事件流**：`onSessionEvent(id, fn)` → onion `SessionNotice` 原样透传。**首个 `snapshot` 帧既是全量历史也是连接就绪信号**（空闲会话在 snapshot 前没有任何状态帧，「连接中→空闲」以 snapshot 到达为准）。帧 `type` 标签（snake_case）：`snapshot` / `durable`（内含 `turn_start` `user_message` `assistant_message` `assistant_attempt` `tool_call` `tool_result` `turn_end`）/ `live`（`AssistantStream` `ToolCallStart`…，`Durable` 帧需跳过）/ `approval_requested` / `plan_review_requested` / `question_requested` / `turn_report` / `session_closed`
- **待回填**：`getPending(id)` → `{kind: approval|plan_review|question}`；答复：`replyApproval` / `replyPlanReview` / `answerQuestion(id, requestId, answers: [{id, selected, custom?}])`
- **元信息**：`listModels()`（provider 分组目录，模型下拉数据源；切换经 `setSessionModel`） / `listAgents()`

渲染层已沉淀的会话状态机（乐观气泡对账、工具行去重、`turn_end` 收束等不变量）见 [chatapp-rendering-design.md](../design/chatapp-rendering-design.md)。

## 7. 事件总线

- `ctx.events.on(type, fn)`：App 级订阅——收**广播事件** + **定向到本 App** 的事件（载荷已剥离 `appId`）。
- `ctx.events.emit(type, e)`：回执类事件（如 `agent.hookResult`）只回宿主，其余广播给全体。
- 已约定事件：`canvas.open { viewId }`（请求壳打开画布 tab，折叠中自动展开）、`canvas.openPath`（打开工作区）、`agent.invoke` / `agent.reply`（App↔Agent 协作）。
- **App 内自定义事件（面板跟随模式）**：`panel.*` 与对应 `surface.*` 是独立组件实例、无法共享 state——持久协同走 `ctx.storage`，瞬时协同走自定义总线事件（如 sessionboard 的 `sessionboard.select {id}` 让 panel.canvas 跟随试验台选中会话）。事件名建议加 App id 前缀避免跨 App 撞名。

## 8. 安装 / 启停 / 卸载

边车 REST（宿主 App 管理面板已内置，也可脚本直调）：

| 操作 | 端点 |
| --- | --- |
| 安装（复制产物目录并登记） | `POST /v1/apps/install` body `{"path": "<产物目录绝对路径>", "overwrite": true}`；已装未 overwrite → 409 |
| 卸载（删目录 + 登记项） | `DELETE /v1/apps/{id}` |
| 启停（持久化，重启保持） | `PUT /v1/apps/{id}/enabled` body `{"enabled": true|false}` |
| 已装清单 | `GET /v1/apps` |
| 静态产物 | `GET /v1/apps/{id}/{*path}`（ESM 相对 import 的解析基准） |

安装后宿主增量装配（无需重启）；禁用只隐藏贡献、模块保留（零成本恢复）。

## 9. 调试

- **双端基址**：web-ui 默认 `http://127.0.0.1:9800`，`?backend=http://host:port` 可覆盖；桌面端由壳动态分配端口经 `backend_base` 注入。App 一律走 `ctx.agent`，**禁止自行判断运行环境或用 Tauri 特权**。
- **版本指纹**：ESM 按 URL 缓存，manifest `version` 变化会带 `?v=` 破缓存——改了代码记得 bump version，否则热重载可能仍跑旧模块。
- **失败可见**：清单非法 / setup 抛错 / 越权注入都会让 App 进入 `failed`（App 管理面板展示原因），不影响其他 App。
- **外联越权**：`ctx.net.fetch` 域不在 `net.allow` 白名单（或未声明 `net` 能力面）时宿主直接抛错，错误信息含 App id 与白名单——修正 manifest 后 bump version 重装。
- ** CORS**：浏览器形态需边车 `ONION_SERVER_CORS_ORIGINS` 包含 web-ui 源。

## 10. 约束清单（评审基线）

1. `setup(ctx)` 只登记 slot 与订阅，不做 DOM/网络副作用；组件惰性 import。
2. React 系依赖一律 external（单实例 React），不打包第二份。
3. 样式限域在自有根类下，颜色引用壳令牌 `var(--*)` 或同数值局部变量，亮/暗主题都要覆盖。
4. App 不直连 WS、不使用 Tauri 特权；外联一律走 `ctx.net.fetch`（自行 `fetch` 第三方必失败）——平台服务一律经 `ctx`；联网 App 已声明 `inject: ["net"]` 并按需收窄 `net.allow`。
5. slot 认领遵循 viewId 全局唯一；引用型面板的 viewId 必须指向本 App（或任一 App）已认领的视图。
