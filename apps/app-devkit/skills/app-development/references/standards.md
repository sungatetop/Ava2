---
title: ava2 App 开发规范（app-devkit 分发副本）
description: 面向 App 开发者的规范总纲（自包含副本）：契约权威链、两种产物形态、manifest/setup/slot/ctx 编程规则（含 ctx.net 统一联网服务）、contributes 能力贡献、安装调试流程、样式令牌与交付自检清单。
keywords: ava2, App 开发规范, AppHost, manifest, setup, slot, ctx.net, contributes, 交付自检
author: Ava Team
date: 2026-11-22
status: done
---

<!-- 本副本随 app-devkit 技能分发，内容自包含：规范全文内联，必要参考资料
     （契约 SPEC references/app-contract.json、图标规范 references/icon-spec.md、
     设计基准 references/DESIGN.md）一并随技能分发，可独立使用。
     权威源：仓库 docs/references/app-dev-standards.md（下文仓库路径均相对 ava2
     仓库根展开）；权威源更新时必须同步本副本与上述参考资料。 -->

# ava2 App 开发规范

本文件是 ava2 App 开发的**规范总纲**：把散落在契约 SPEC、开发指南、设计系统中的开发规则收敛为一处，供开发者按图索骥。细节以权威来源为准，本文与其冲突时**以下列权威链为准**。

## 1. 契约权威链（先读契约，勿凭记忆）

按优先级从高到低，冲突时以更靠前者为准：

| 顺位 | 来源 | 角色 |
| --- | --- | --- |
| 1 | 本技能 `references/app-contract.json` | 机器可读 SPEC（manifest 字段 / 枚举 / slot / ctx / 事件 / 端点 / 约束），校验脚本唯一基准；仓库权威源 `contracts/app-contract.json` |
| 2 | 仓库 `src/host/types.ts` | 类型单一来源（`@ava-ai/app-sdk` 即它的 d.ts 投影，type-only；代码文件，按需读仓库） |
| 3 | 本技能 `references/standards.md`（即本文件） | 规范总纲（§2.4 内联两种形态的完整骨架示例） |

防漂移校验：在 ava2 仓库根执行 `npm run check:contracts` 交叉校验 SPEC ↔ `types.ts` / `manifest.ts` / `rest/apps.rs`，并校验全部 `apps/*/manifest.json` 与 `dist/manifest.json`。**0 error 才算交付。**

## 2. 开发流程总览

```
定目录与 id → 选形态 → 写 manifest.json → 写 setup(ctx) → 安装验证 → bump version 交付
```

### 2.1 目录与 id

- 源码位置固定为 `ava2/apps/<id>/`，`<id>` 必须与 `manifest.id` **完全一致**。
- `id` 规则：`^[A-Za-z0-9._-]{1,64}$`，推荐 kebab-case。

### 2.2 选形态

按 **UI 复杂度与产品要求**决策，不是按「能不能省掉构建」：

| 形态 | 适用场景 | 产物 | 安装源 |
| --- | --- | --- | --- |
| A · 裸 ESM | 纯能力 App（hooks / mcp / skills 贡献，无 UI 或 UI 只是配角）；极简小工具（单视图、静态布局、无列表无动效，如时钟、开关面板） | `manifest.json` + `app.js` | App 源码目录 |
| B · esbuild 构建型 | **一切面向用户的产品级 UI**：数据密集（榜单 / 卡片 / 图表 / 表格）、多 tab 或多面板联动、需要动效与过渡（framer-motion）、组件复用与状态管理 | `src/main.tsx` + `build.mjs` → `dist/main.js` | **`dist/`** |

判断信号：UI 总量 ≤ ~80 行 JSX 等价物、无列表渲染、无视图切换、无动效 → 可选 A；任一信号（列表渲染 / 条件视图切换 / 需要动效 / UI 超 ~100 行）→ **必须 B**。

**默认规则：面向用户的展示型 App 一律选形态 B。** 用形态 A 实现榜单 / 卡片 / 动效只能手写 DOM/CSS，必然产出朴素平铺的列表，评审不过。

构建型 `build.mjs` 关键不变式——react 系一律 external（单实例 React）：

```js
await esbuild.build({
  entryPoints: ["src/main.tsx"],
  bundle: true, format: "esm", target: "es2022", jsx: "automatic",
  outfile: "dist/main.js",
  external: ["react", "react-dom", "react/jsx-runtime", "react/jsx-dev-runtime", "react-dom/client"],
});
```

类型引用：App 的 `package.json` 声明 `@ava-ai/app-sdk` 依赖（仓库内 `file:../../packages/app-sdk` workspace 链接，外部 `npm i -D @ava-ai/app-sdk`），tsc 经 node_modules 解析到包 d.ts，零运行时依赖：

```json
{ "dependencies": { "@ava-ai/app-sdk": "file:../../packages/app-sdk" } }
```

### 2.3 manifest 模板

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
  "inject": ["slots", "agent"],
  "surfaces": ["surface.main", "nav.sidebar"],
  "net": { "allow": ["api.example.com"] },
  "contributes": { "apps": [{ "id": "main", "entry": "app.js" }] }
}
```

硬性规则：

- `hostApi` 一律写 `2`（当前上限即 2，超出安装被拒 `app_host_api_unsupported`）。
- 入口：顶层 `entry` 或 `contributes.apps[].entry` **至少声明一个**；必须是 App 根目录内的相对路径且文件真实存在。纯能力 App（无 UI）也需一个空 `setup` 入口。
- `inject` 只填要用的能力面（`slots` / `events` / `agent` / `storage` / `net`），未声明即访问会响亮报错。
- `net`：联网服务授权（`inject` 含 `"net"` 时生效）。`allow` 是裸域名数组（host 等于域或为域子域即放行）；**缺省 = 全部 https 公网**，声明即收窄为白名单。越权（域不在白名单）宿主直接抛错，不静默失败。
- `surfaces` 只填要注入的 slot key，越权注入加载期报错。
- 多页面用 `contributes.apps` 逐页 `{ id, entry, inject?, surfaces? }`，页 `id` 在 App 内唯一。
- **展示字段**：`description` / `author` / `homepage` / `license` / `icon` 均可选、缺省不渲染。`author` 在详情面板展示「开发者：xxx」，`license` 展示「协议：xxx」（推荐 SPDX 标识符），`homepage` 仅 `http(s)://` 前缀渲染为链接；`icon` 是应用形象的**权威声明**（AppIcon，24×24 SVG path，规范见本技能 `references/icon-spec.md`，运行时有校验），未声明时宿主回落遍历 slot 内 icon——纯能力 App 声明 `icon` 后应用宫格也能有图标。
- **改代码必 bump `version`**：入口 URL 带 `?v=<version>` 破 ESM 缓存，不 bump 可能仍跑旧模块。

### 2.4 两种形态的完整骨架示例（照此起步）

#### 形态 A · 裸 ESM（极简小工具）

目录结构：

```
my-app/
├── manifest.json        # 顶层 "entry": "app.js"
└── app.js
```

app.js（纯 DOM + 样式注入 + storage 持久化，完整可跑）：

```js
const CSS = `
.todo-root { display: flex; flex-direction: column; gap: var(--ava-space-4, 16px);
  padding: var(--ava-space-4, 16px); height: 100%; overflow-y: auto;
  color: var(--ava-label-primary); font: 13px/20px var(--ava-font-ui); }
.todo-head { font-size: 15px; font-weight: 600; }
.todo-row { display: flex; align-items: center; gap: var(--ava-space-2, 8px);
  height: 32px; padding: 0 var(--ava-space-3, 10px);
  border-radius: var(--ava-radius-sm, 10px); transition: background 150ms ease; }
.todo-row:hover { background: var(--ava-interactive-bg-hover); }
`;

export function setup(ctx) {
  // 样式限域在自有根类下，setup 期一次性注入
  const style = document.createElement("style");
  style.textContent = CSS;
  document.head.appendChild(style);

  function rowEl(text, done) {
    const row = document.createElement("div");
    row.className = "todo-row";
    row.textContent = (done ? "✓ " : "· ") + text;
    return row;
  }

  async function Main() {
    const root = document.createElement("div");
    root.className = "todo-root";
    const head = document.createElement("div");
    head.className = "todo-head";
    head.textContent = "待办清单";
    root.appendChild(head);

    // storage：跨 tab 切换/卸载持久化（契约统一 async）
    const items = (await ctx.storage.get("items")) ?? [];
    for (const it of items) root.appendChild(rowEl(it.text, it.done));
    return root;
  }

  const page = ctx.slots.inject("surface.main", {
    viewId: "my-todo", title: "待办",
    component: async () => ({ default: Main }),        // 惰性工厂
  });
  const nav = ctx.slots.inject("nav.sidebar", {
    order: 10, viewId: "my-todo", title: "待办",
  });

  return {
    dispose() {
      nav.dispose(); page.dispose();
      style.remove();                                  // 释放自有资源
    },
  };
}
```

#### 形态 B · React 构建型（产品级 UI 首选）

目录结构：

```
my-app/
├── manifest.json        # contributes.apps[].entry（或顶层 entry）指向 dist/main.js
├── package.json         # 见下方依赖声明
├── tsconfig.json        # 见下方类型解析
├── build.mjs            # esbuild：react 系一律 external（§2.2 已给出完整文件）
└── src/
    ├── main.tsx         # setup(ctx) + 组件（见下）
    └── styles.ts        # CSS 字符串（组件 <style> 注入，限域 .hot-root）
```

package.json 与 tsconfig.json：

```json
// package.json —— 依赖走仓库内 SDK（file: 链接），构建工具为 devDependency
{
  "name": "my-app", "private": true,
  "scripts": { "build": "node build.mjs" },
  "dependencies": { "@ava-ai/app-sdk": "file:../../packages/app-sdk" },
  "devDependencies": { "esbuild": "^0.25.0" }
}
```

```json
// tsconfig.json —— 类型解析到宿主契约，零运行时依赖
{
  "compilerOptions": {
    "target": "ES2022", "module": "ESNext", "moduleResolution": "bundler",
    "jsx": "react-jsx", "strict": true, "noEmit": true,
    "paths": { "@ava-ai/app-sdk": ["../../src/host/types.ts"] }
  },
  "include": ["src"]
}
```

src/main.tsx（组件化 + ctx 闭包捕获 + ctx.net 联网 + 三态，骨架）：

```tsx
import { useEffect, useState } from "react";

const CSS = `
.hot-root { display: flex; flex-direction: column; height: 100%;
  padding: var(--ava-space-4, 16px); gap: var(--ava-space-3, 10px);
  color: var(--ava-label-primary); font: 13px/20px var(--ava-font-ui); }
.hot-row { display: flex; align-items: center; gap: var(--ava-space-2, 8px);
  height: 32px; padding: 0 var(--ava-space-3, 10px); border-radius: var(--ava-radius-sm, 10px);
  transition: background 150ms ease; cursor: pointer; }
.hot-row:hover { background: var(--ava-interactive-bg-hover); }
.hot-rank { width: 24px; text-align: center; font-weight: 600; color: var(--ava-label-tertiary); }
.hot-rank.top { color: var(--ava-state-business-primary); }        // 前三名品牌色
.hot-hot { margin-left: auto; color: var(--ava-label-secondary); font-variant-numeric: tabular-nums; }
`;

type Item = { title: string; hot: number };

function Row({ item, rank }: { item: Item; rank: number }) {
  return (
    <div className="hot-row" onClick={() => window.open(item.url, "_blank")}>
      <span className={`hot-rank${rank <= 3 ? " top" : ""}`}>{rank}</span>
      <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        {item.title}
      </span>
      <span className="hot-hot">{item.hot >= 1e8 ? `${(item.hot / 1e8).toFixed(1)} 亿` : `${Math.round(item.hot / 1e4)} 万`}</span>
    </div>
  );
}

export function setup(ctx: any) {
  function Main() {
    const [items, setItems] = useState<Item[] | null>(null); // null = 加载中（骨架屏）
    const [err, setErr] = useState("");

    useEffect(() => {
      let alive = true;
      // 联网唯一出口：ctx.net.fetch（manifest 已声明 inject: ["net"]）
      ctx.net.fetch({ url: "https://api.example.com/hot" })
        .then((r: any) => {
          if (!alive) return;
          if (r.status === 200) setItems(JSON.parse(r.body).items);   // 状态码原样透传
          else setErr(`上游 ${r.status}`);
        })
        .catch((e: unknown) => alive && setErr(String(e)));
      return () => { alive = false; };                     // 请求/订阅必须可清理
    }, []);

    if (err) return <div className="hot-root">加载失败：{err}</div>;
    if (!items) return <div className="hot-root">加载中…</div>; // 实际交付用骨架屏块（见 DESIGN.md §6）
    return (
      <div className="hot-root">
        <div style={{ fontSize: 15, fontWeight: 600 }}>热榜聚合</div>
        {items.map((it, i) => <Row key={it.title} item={it} rank={i + 1} />)}
      </div>
    );
  }

  const page = ctx.slots.inject("surface.main", {
    viewId: "my-hot", title: "热榜",
    component: async () => ({ default: Main }),            // 惰性工厂
  });
  const nav = ctx.slots.inject("nav.sidebar", { order: 12, viewId: "my-hot", title: "热榜" });
  return { dispose: () => { page.dispose(); nav.dispose(); } };
}
```

> 完整产品级 UI（分段控件 / 排名徽标 / 骨架屏 / 空错态 / 亮暗令牌）按 `references/DESIGN.md` 配方补齐。

## 3. setup(ctx) 编程规范

`setup(ctx)` 是 App 唯一入口 API，**组件拿不到 ctx**——在 setup 里用闭包把需要的回调和 ctx 捕获后传给组件。

```ts
export function setup(ctx) {
  function Main() {
    const [model, setModel] = React.useState("");
    React.useEffect(() => {
      let alive = true;
      ctx.agent.listModels().then((r) => alive && setModel(r.defaultModel));
      const off = ctx.events.on("canvas.open", () => {});
      return () => { alive = false; off.dispose(); };   // 订阅必须可清理
    }, []);
    return <div className="myapp-root">…</div>;
  }
  const d1 = ctx.slots.inject("nav.sidebar", { order: 10, viewId: "my-app", title: "我的应用" });
  const d2 = ctx.slots.inject("surface.main", {
    viewId: "my-app", title: "我的应用",
    component: async () => ({ default: Main }),          // 必须惰性工厂
  });
  return { dispose: () => { d1.dispose(); d2.dispose(); } };
}
```

不变量（逐条为评审基线）：

1. setup 只登记 slot / 订阅，**不做 DOM 或网络副作用**。
2. `component` 一律是惰性工厂 `async () => ({ default: Comp })`。
3. `events.on` 返回的 Disposable 在 hooks 中必须以 `() => d.dispose()` 作为 cleanup 返回。
4. 平台服务一律经 `ctx`（`ctx.agent` / `ctx.events` / `ctx.storage` / `ctx.net`）；App **不直连 WS、不用 Tauri 特权**；外联**一律走 `ctx.net.fetch`**，禁止自行 `fetch` 第三方站点（浏览器 CORS 白名单不可控，必然失败）。
5. 配置完全由 App 自管（宿主零策略）：存储、校验、回显自理；需要「设置」入口时在 setup 期 `ctx.registerSettingsPanel(() => import("./settings.js"))` 登记，宿主只提供入口按钮 + 惰性挂载 + 错误隔离。
6. App 卸载 / 禁用时宿主自动撤回 slot 与订阅；setup 返回的 `dispose` 用于释放自有资源。

## 4. slot 注入规范（六类）

| slot key | 形态 | viewId 语义 | 壳中落点 |
| --- | --- | --- | --- |
| `surface.main` | 认领型 | 路由键，**全局唯一**（冲突加载期报错） | 主区 tab 舞台（可脱离 tab 悬浮） |
| `surface.canvas` | 认领型 | 路由键，**全局唯一** | 画布第二舞台 tab |
| `nav.sidebar` | 引用型 | 指向本 App 的主区视图 | 左侧导航（32px 行，图标 18px） |
| `panel.sidebar` | 引用型 | 跟随 `viewId` 对应的主区视图 | 侧栏下方面板区 |
| `panel.canvas` | 引用型 | 跟随 `viewId` 对应的画布视图 | 画布舞台下方面板区 |
| `user.menu` | 引用型 | 无（菜单项非视图） | 侧边栏 user 栏上浮菜单的 App 区 |

- `surface.*` spec：`{ viewId, title, component }`（canvas 另有 `icon?` / `description?`）。
- `nav.sidebar` spec：`{ order, viewId, title, icon? }`，按 `order` 升序。
- `panel.*` spec：`{ order, viewId, title, component }`，按 `viewId` 过滤 + `order` 排序。
- `user.menu` spec：`{ id, title, icon?, order?, action? }`，按 App 名分组、组内 `order`（缺省 100）排序；`action` 为纯数据动作（`openView` / `openSettings` / `event`），宿主解释执行。
- 同一 App 可对同一 viewId 注入多次（如多面板），最后一个撤回才释放认领。
- **状态驻留范围**：舞台折叠/展开不卸载组件；**tab 间切换会卸载**非激活视图，悬浮与还原也是一次卸载/重挂——跨切换/悬浮需保留的状态进 `ctx.storage` 或可由 REST 重建。
- `panel.*` 与对应 `surface.*` 是独立组件实例、无法共享 state：持久协同走 `ctx.storage`，瞬时协同走自定义总线事件（事件名加 App id 前缀防撞名）。

## 5. ctx 能力面规范

| 面 | API | 要点 |
| --- | --- | --- |
| `ctx.manifest` | 只读清单 | 含宿主注入的 `baseUrl` 与 `pageId`；App 不得自填 `baseUrl` |
| `ctx.slots.inject(key, spec)` | → Disposable | 见 §4 |
| `ctx.events.on / emit` | → Disposable / void | 见下事件总线 |
| `ctx.agent` | AgentChannel | 平台 Agent 通道（REST + WS 收敛封装，每会话一条 WS 多路复用），**不直连 WS** |
| `ctx.storage.get / set` | async KV | localStorage 命名空间 `app:<id>:`，契约统一异步 |
| `ctx.net.fetch(input)` | async 联网 | 统一联网服务（详见下「联网服务」）；须 `inject` 声明 `"net"`，域受 `manifest.net.allow` 约束 |
| `ctx.registerSettingsPanel(factory)` | void | setup 期登记设置面板惰性工厂（`() => Promise<{ default: Comp }>`）；宿主只提供「设置」入口 + 惰性挂载 + 错误隔离，表单 / 存储 / 校验全部由 App 自理 |

**联网服务（ctx.net，统一外联出口）**：

- 请求：`ctx.net.fetch({ url, method?, headers?, body?, encoding? })`，`method` 仅 `GET`（缺省）/ `POST`，`encoding` 为 `"text"`（缺省）/ `"dataUrl"`（body 走 base64 data URL，用于二进制上传）。
- 响应：`{ status, contentType, body }`；**上游状态码原样透传**（404 / 405 等按上游语义处理，勿当作宿主错误）。
- 授权：`inject` 声明 `"net"`；`manifest.net.allow` 缺省 = 全部 https 公网，声明即收窄。越权（未声明能力面或域不在白名单）宿主直接抛错，错误信息含 App id 与白名单内容。
- 安全底线（服务端 `/v1/net/fetch` 强制）：仅 https 公网目标（拒绝 localhost / 私网 / 环回 / 链路本地），重定向逐跳复检，请求头白名单（accept / authorization / content-type / user-agent），响应 ≤ 4 MiB，超时 15 s。
- App 内**禁止**自行 `fetch` 第三方站点：上游 CORS 只回显自家域，浏览器直连必失败；`ctx.net.fetch` 经边车转发是唯一合法通道。

**Agent 通道要点**（完整方法清单见 SPEC `agentChannel.methods`）：

- `listSessions()` 返回顺序未定义，勿依赖；列表项不含 `model` / `approvalPreset`，需走 `getSession(id)` 详情。
- `createSession(cwd?)` 即工作区切换：权限根在会话创建期固定，跨工作区 = 新会话。
- `onSessionEvent(id, fn)` 原样透传 onion `SessionNotice`；**首个 `snapshot` 帧既是全量历史也是连接就绪信号**。
- 会话运行期切换：`setSessionModel` / `setApprovalPreset`（preset ∈ `workspace-write` / `auto` / `danger-full-access`），均返回更新后的 SessionInfo。
- 待回填：`getPending(id)` → `{kind: approval|plan_review|question}`，答复走 `replyApproval` / `replyPlanReview` / `answerQuestion`。

**事件总线**：

- `on`：收广播事件 + 定向到本 App 的事件；`emit`：回执类（如 `agent.hookResult`）只回宿主，其余广播。
- 已约定事件：`canvas.open { viewId }`、`canvas.openPath { path }`、`agent.invoke` / `agent.reply`。

## 6. 能力贡献（contributes）规范

hostApi≥2 支持五类贡献，彼此独立可组合；skills / mcpServers / hooks 由边车在**每次会话装配期**注入，安装 / 启停后新会话即时生效，无需重启。

| 类别 | 形态 | 关键规则 |
| --- | --- | --- |
| `apps` | `[{ id, entry, inject?, surfaces? }]` | 前端宿主装配（多页面）；页 id 在 App 内唯一 |
| `skills` | `["目录"]` 或 `["x/SKILL.md"]` 或 `{ path }` | 目录按技能根扫描；注册为独立 provider（`app:<id>:<序号>`） |
| `mcpServers` | 内联 `[{ name, command?/url?, args?, env? }]` 或 `{ path: "mcp.json" }` | command 即 stdio、url 即 streamable-http，二者其一；与既有 server 重名时跳过 |
| `hooks` | 内联 `[{ event, matcher?, handler }]` 或 `{ path: "hooks.json" }` | Claude Code 兼容；边车**只执行 `command` 处理器**，`bus` 装配期跳过 |
| `agents` | 预留 | 声明不报错也不生效 |

- 命令可用 `${CLAUDE_PLUGIN_ROOT}` 引用 App 内脚本（装配期替换为 App 根目录）；hooks 另支持 `${CLAUDE_PROJECT_DIR}`（会话工作目录）。
- **所有路径必须落在 App 根目录内**（拒绝绝对路径与 `..`），安装期校验存在性，不满足直接拒绝安装（`app_contributes_invalid`）。
- hook 脚本协议：stdin 收 JSON 载荷，stdout 输出决策 JSON（`hookSpecificOutput.additionalContext` 注入模型输入，多钩子产出全部拼接；`decision:"block"` + `reason` 拦截）；多个钩子输出按最严决策生效。

## 7. 安装 / 启停 / 调试规范

安装源是**预构建产物目录**，安装期不做 npm / 构建步骤：裸 ESM 装源码目录，构建型装 `dist/`。安装 / 启停 / 卸载的 REST 命令见 SKILL.md §8（操作入口）。

调试要点：

- 端口：web-ui 默认 `9800`（桌面端由壳动态分配，经 `backend_base` 注入）；App 一律走 `ctx.agent`，禁止自行判断运行环境。
- 安装后宿主增量装配，**无需重启**；在 `http://localhost:1420/` 观察导航 / 主区 / 画布。
- **改代码必 bump version**（ESM 按 URL 缓存，`?v=` 破缓存）。
- 失败可见：清单非法 / setup 抛错 / 越权注入 → App 进入 `failed`（App 管理面板展示原因），不影响其他 App。
- CORS：浏览器形态需边车 `ONION_SERVER_CORS_ORIGINS` 包含 web-ui 源。

### 错误码速查

| 错误码 | 原因 / 处理 |
| --- | --- |
| `app_manifest_missing` / `app_manifest_invalid` | 目录里没有 `manifest.json`，或不是合法 JSON 对象 |
| `app_id_missing` / `app_id_invalid` | `id` 缺失、含非法字符或超 64 字符 |
| `app_name_missing` | `name` 为空 |
| `app_host_api_unsupported` | `hostApi` 超过宿主上限（当前 2） |
| `app_entry_missing` / `app_entry_invalid` / `app_entry_not_found` | 缺入口、入口是绝对路径或含 `..`、入口文件不存在（构建型请装 `dist/`） |
| `app_contributes_invalid` | `contributes` 路径越界或不存在、MCP / hooks 配置无法解析 |
| `app_exists` | 已安装；覆盖请 `overwrite: true` |
| `app_path_invalid` | 安装源不可用（必须是含 `manifest.json` 的目录） |

**运行期外联报错（非 app_\* 错误码）**：`App「<id>」外联越权：<host> 不在 net.allow 白名单（...）` ——宿主在 `ctx.net.fetch` 调用时抛出。修正方向：manifest 增补 `inject: ["net"]`（漏声明），或把目标 host（及其父域）加入 `net.allow`，bump version 后 overwrite 重装。

## 8. 样式与设计规范

生成或改造任何 App UI 前，先读本技能分发的两份基准资料：`references/DESIGN.md`（App UI 设计指南：设计气质 / 令牌速查 / 排版 / 组件配方 / 数据可视化锚点 / 反模式禁令）与 `references/icon-spec.md`（App 图标绘制指南）。**美学硬要求（三段式结构 / 视觉锚点 / 微交互 / 反面清单）全部以 DESIGN.md 为准**，本节只列工程约束：

- **限域**：样式收敛在自有根类下（如 chatapp 的 `.cc-root`），构建型 App 的 CSS 打包为样式字符串、由组件 `<style>` 注入，不污染壳与其他 App。
- **令牌**：颜色 / 字号 / 间距 / 圆角一律消费 `var(--ava-*)`（壳语义令牌，亮暗两套自动翻转）或 App 本地令牌，**禁止硬编码色值字号**；App 不得改写壳令牌。
- **亮暗主题都要覆盖**：令牌由 `data-theme="light|dark"` 整体翻转，App 内不要写死仅亮色可用的组合。
- **字体**：`--ava-font-ui` / `--ava-font-mono` 双字体栈由壳统一下发，App 不要自备字体栈；禁前置 `system-ui` / `PingFang SC`（会拦截中文字形，目标字体永不生效）。
- **图标**：AppIcon 单色 24×24 网格，见 `references/icon-spec.md`，运行时有校验。

## 9. 交付前自检清单

```powershell
cd <repo-root>   # ava2 仓库根
npm run check:contracts    # SPEC 防漂移 + 全部 manifest 校验，0 error 才算交付
```

逐条自检（对齐 SPEC `constraints`）：

1. [ ] 形态选对了：面向用户的展示型 UI 用形态 B（React 构建型），形态 A 只用于纯能力 App / 极简小工具。
2. [ ] 达到产品级观感：已读 `references/DESIGN.md`，三段式结构、视觉锚点（排名徽标 / 徽章 / 语义 pill）、微交互过渡齐备，§8 反面清单零命中。
3. [ ] `setup(ctx)` 只登记 slot 与订阅，无 DOM / 网络副作用；组件惰性 import。
4. [ ] React 系依赖一律 external，未打包第二份 React。
5. [ ] 样式限域在自有根类下，颜色 / 字号全走令牌，亮暗主题都已覆盖。
6. [ ] 平台服务一律经 `ctx`：不直连 WS、外联一律走 `ctx.net.fetch`（自行 `fetch` 第三方必失败）、不用 Tauri 特权；联网 App 已声明 `inject: ["net"]` 且按需收窄 `net.allow`。
7. [ ] viewId 全局唯一；引用型面板的 viewId 指向已认领的视图。
8. [ ] `contributes` 路径全部落在 App 根目录内且文件真实存在。
9. [ ] `version` 已 bump（相对上次交付）。
10. [ ] 真实环境安装 → 启停 → 卸载全流程通过，冒烟截图留证。
