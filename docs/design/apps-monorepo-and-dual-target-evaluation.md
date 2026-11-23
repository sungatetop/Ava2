---
title: apps 目录 + App 单元化 + web/桌面双端支持 方案评估
description: 评估 ava2 引入 apps/ 独立 App 开发目录后的工程组织形态（monorepo 粒度、TypeScript 构建、运行时动态加载）与 web-ui / Tauri 桌面双端共享架构，给出推荐方案与待决问题，作为宿主内核与 App SDK 落地前的决策底稿。
keywords: ava2, apps, monorepo, app-contract, 动态加载, import map, 单实例 React, web-ui, Tauri, 双端
author: AI 助手
date: 2026-11-07
updated: 2026-11-11
status: done
category: design
---

# apps 目录 + App 单元化 + web/桌面双端支持 方案评估

## 1. 目标与约束

把 ava2 从「单前端 + 边车」扩展为「宿主 + 多个独立 App」的结构，并让宿主同时服务 **web-ui**（浏览器）与 **桌面端**（Tauri 壳）两种形态。

明确约束：

1. `apps/` 是每个独立 App 的开发目录（如 `apps/chatapp/`），每个 App 独立开发、独立构建。
2. App 使用 **TypeScript**。
3. Tauri 只做**桌面端薄壳**（窗口 + 边车 + 端口注出），不含任何 App 逻辑。
4. AppHost 契约沿用 ava 冻结版的 `setup(ctx)` 模型（清单一二四原样复用，清单三 AgentChannel 适配 onion，见 [app-host-contract-onion-adaptation.md](./app-host-contract-onion-adaptation.md)）。
5. 单一 App 产物，web-ui 与桌面端**共享同一份 host 前端**，不维护两套。

## 2. 术语澄清：什么叫「每个 App 是独立的 monorepo」

「monorepo」标准含义是**单仓库多包**，与「每个 App 独立」存在术语张力。这里有两种可解释：

| 解释 | 实际形态 | 依赖共享 | 构建关系 |
| --- | --- | --- | --- |
| A. 目录级独立（推荐） | `apps/` 下每个 App 一个目录，各自独立 `package.json` / 独立构建；宿主按产物静态 serve | **不共享** node_modules（react 由 import map 供给） | App 不进入宿主构建图 |
| B. 依赖级 monorepo（pnpm workspace） | 每个 App 声明为 workspace member，共享 `node_modules` 与工具链 | 共享（可能破坏「运行时动态加载」边界） | 宿主可引用 App 源码 |

**关键判断**：App 的运行时形态是「外部 ESM bundle，宿主 `import()` 动态加载，react 由 import map 单实例供给」。这决定了 App 的依赖**必须 external（react 系）**，其余依赖构建期内联。因此 App 天然是**自包含的独立构建单元**，不需要与宿主共享 node_modules。

结论：**推荐解释 A（目录级独立）**。`apps/` 只是把 App 放进同一仓库便于协作与统一工具链，不是依赖共享的 workspace。App 与宿主之间唯一的耦合点是**共享类型包**（§4 的 `@ava2/app-sdk`），且该包只做 devDependency 类型引用，不进入运行时。

## 3. App 单元形态

每个 App 是一个目录，包含四类文件：

```
apps/chatapp/
├── package.json          # 独立构建脚本 + 对 @ava2/app-sdk 的 devDependency
├── tsconfig.json         # 继承共享 tsconfig.base.json
├── vite.config.ts        # lib mode，react/react-dom external
├── manifest.json         # App 契约清单（inject/surfaces/config/contributes）
└── src/
    ├── app.ts            # export function setup(ctx)
    └── chat-view.tsx     # 惰性组件（工厂返回组件函数，不返回 element）
```

- **入口**：`setup(ctx)` 只登记工厂与订阅，不碰 DOM（对齐冻结契约 §2.1）。
- **manifest**：`id` / `name` / `version` / `hostApi` / `inject` / `surfaces` / `config` / `contributes`。
- **构建产物**：`app.js` + 惰性 chunk，react/react-dom **external**，由宿主 import map 供给。

### 3.1 App SDK（共享类型包，App 与宿主的契约桥）

`ctx` 的类型（`AppContext` / `AppManifest` / `AgentChannel` / `Disposable` 等）必须被 App 与宿主共同引用，否则契约类型会两边漂移。抽一个 **`@ava2/app-sdk`** 类型包：

- 只含类型 + 常量（slot key、inject key、事件名），**零运行时逻辑**。
- App 以 `devDependencies` 引用（`import type { AppContext } from "@ava2/app-sdk"`），运行时零成本。
- 宿主持有权威实现，`@ava2/app-sdk` 是类型单一来源（对应 ava 原型 `src/host/types.ts` 的地位）。

包位置二选一：`packages/app-sdk/`（独立发布）或 `src/host/types.ts`（宿主内导出）。**原型阶段建议先放宿主 `src/host/types.ts`，App 用相对路径或 tsconfig path 别名引用**，等 App 数量多了再抽独立包。

## 4. App 构建与分发：安装式（已落地，取代初版「静态 serve 进 public」）

App 用 TS，就必须构建（不能像 ava 原型那样手写 ESM）。初版评估推荐「路线 1：每 App `vite build` → 产物落宿主 `public/` 静态 serve」，宿主内核落地后**升级为安装式分发**（对齐 dsh 插件安装模型）：App 编译产物不再进宿主 public，而是作为**预构建产物目录**经边车安装进独立 App 仓库。

| 环节 | dsh 插件安装 | ava2 App 安装（已实现） |
| --- | --- | --- |
| 交付物 | 预构建 bundle（package.json + 编译产物） | 预构建 App 目录（`manifest.json` + ESM 产物 + 资源） |
| 安装动作 | `dsh plugin add ./pkg` 复制进 profile | `POST /v1/apps/install {path,overwrite}` 复制进 `<ONION_HOME>/apps/<id>/` |
| 登记表 | profile 有序 bundles 清单 | `state.json`（enabled + installed_at_ms）；**未登记目录不被发现** |
| 加载 | loader 按层加载 | 宿主 `GET /v1/apps` 发现 → `import()` 动态装配 |
| 构建期脚本 | GitHub 源码包需 prepare + allowBuilds 授权 | **不支持源码包**：安装期零构建零脚本，只收预构建产物（等价问题直接回避） |

最终形态要点：

1. **App = 预构建产物目录**：各 App 在 `apps/<id>/` 独立 TS 构建（lib mode，react 系 external），产出目录即安装包。
2. **边车托管仓库**：onion-server 提供 `/v1/apps` 六端点（list / manifest / install / uninstall / enabled / asset），仓库目录由 `ONION_APPS_DIR` 指定（缺省 `<data_home>/apps`）。
3. **登记表唯一权威**：`state.json` 记录启停态与安装时间，宿主发现只走 `GET /v1/apps`，不扫本机目录、不读 public/；启停态持久化在边车，宿主重启保持。
4. **产物经边车供给**：`GET /v1/apps/{id}/{path}` 带白名单 id、canonicalize 路径逃逸校验、MIME 映射、no-store + nosniff、manifest 64KiB / 单产物 16MiB 上限；web-ui 与桌面端同一 URL 空间。
5. **安装安全面**：安装期不执行任何脚本；拒绝 symlink；重复安装同 id 返回 409（需 `overwrite: true`）。

### 4.1 关键不变式（必须继承 ava 原型的教训）

1. **单实例 React**：host alias、JSX shim、import map 三方解析到**同一 URL**；vendor 产物放 `src/vendor/`（或独立 `vendor/` 目录），**不放 `public/`**（双 URL 陷阱）。
2. **App 的 react 必须 external**：App 构建不内联 react，运行时由宿主 import map 供给，否则双 React → hooks 崩溃。
3. **当前 vite.config.ts 用了 `@vitejs/plugin-react`，与上述方案冲突**——该插件会触发 optimizeDeps 预打包 react → 双实例。宿主侧须切换到 ava 原型验证过的「esbuild `jsx: "automatic"` + 本地 jsxImportSource shim」方案（详见 `ava/prototypes/app-host-proto/vite.config.ts`）。
4. **组件工厂只返回组件函数**、`events.on` 的 Disposable 在 hooks 中以 `() => d.dispose()` 作 cleanup（两条 App 侧契约警示）。

## 5. web-ui vs 桌面端：单一 host，环境适配层

两者共享同一份 host 前端与同一份 App 产物，唯一差异是「后端基地址从哪来」。引入 `env.ts` 适配层收敛：

| 维度 | web-ui | 桌面端 |
| --- | --- | --- |
| host 前端 | 同一份 `src/` | 同一份 `src/` |
| 运行环境 | 浏览器 | Tauri WebView |
| 后端基地址 | `?backend=` 查询参数或默认 `http://127.0.0.1:9800` | Tauri `backend_base` command |
| 边车 | 手动 / 脚本起 onion-server | Tauri `setup` 自动 spawn |
| 持久化 | localStorage | localStorage（WebView 支持），不引入 Tauri fs 以保持一致性 |
| 文件系统 | 走 onion-server 端点 | 走 onion-server 端点（与 web-ui 对齐，不用 Tauri fs 特权） |

`env.ts` 职责：探测运行环境 → 注入 `backendBase`。web-ui 读取 `?backend=` / 默认端口；桌面端通过 `invoke("backend_base")` 拿 Tauri 注出的端口。其余代码对运行环境无感。

**桌面端不引入 Tauri 特有特权能力**（fs、shell 等）作为 App 通道，是刻意约束：保证「App 在 web-ui 与桌面端行为一致」，避免 App 依赖只在桌面端存在的 Tauri API 而导致 web-ui 崩溃。

## 6. 建议目录结构

```
ava2/
├── apps/                     # 独立 App（目录级独立，各自构建；产物目录即安装包）
│   └── hello/                # 安装源样例（manifest.json + app.js）
│       ├── manifest.json
│       └── app.js
├── src/                      # app-host 宿主前端（web-ui 与桌面共享）
│   ├── host/                 # registry / loader / apps 仓库客户端 / agent / env
│   ├── env.ts                # 运行环境适配（backend_base）
│   ├── App.tsx               # 壳（自注册 home + App 管理安装面板）
│   └── vendor/               # 基线依赖 ESM（react 系，构建期产出）
├── src-tauri/                # 桌面薄壳（窗口 + 边车 + backend_base + CORS 注入）
└── crates/                   # onion-server 边车（含 /v1/apps 安装仓库）
```

## 7. 待决问题

| # | 问题 | 选项 | 影响 |
| --- | --- | --- | --- |
| D1 | 「独立 monorepo」确认解释 A（目录级独立）还是 B（pnpm workspace） | A 推荐 | 决定 package.json 层级与构建编排 |
| D2 | App SDK 放宿主内 `src/host/types.ts` 还是独立 `packages/app-sdk` | 原型阶段放宿主内 | 决定 App 引类型的方式 |
| D3 | App 是否允许非 react 的 npm 依赖 | 允许（构建期内联）vs 禁止（只用 import map vendor） | 决定 App 构建复杂度 |
| D4 | App 产物由谁 serve | **已决：onion-server `/v1/apps/{id}/{path}`**（安装式分发） | 产物经边车供给，web-ui 与桌面端同一 URL 空间 |
| D5 | vendor 基线依赖范围 | 仅 react 系 vs 扩展 framer-motion / zustand | 决定 import map 规模 |
| D6 | 构建编排工具 | 根 scripts 批量 build/watch vs turbo/nx | 决定 dev 体验与复杂度 |

## 8. 实施路径与进度

1. **切换宿主 vite 配置（已完成）**：弃 `@vitejs/plugin-react`，落 ava 原型的 esbuild JSX + vendor shim 方案（D3/D5 的地基）。
2. **迁移宿主内核（已完成）**：ava 原型 `src/host/`（registry / loader / lazy-cache / types）迁入 `ava2/src/host/`，`agent.ts` 对接 onion `/v1` 契约，`env.ts` 落双端适配（`?backend=` / Tauri `backend_base`）。
3. **安装式分发落地（已完成）**：onion-server 新增 `/v1/apps*` 安装仓库（登记表 + 产物供给），宿主 loader 改为仓库发现，App 管理面板支持输入路径安装 / 启停 / 卸载；冒烟 App `apps/hello/` 作为安装源样例。
4. **chatapp 试点 + task-manager 迁移（已完成）**：`apps/chatapp/` 独立 TS 构建（package.json + tsconfig + build.mjs，react external，esbuild 产出 `dist/`）→ 经 `POST /v1/apps/install` 安装（overwrite 覆盖重装），验证「独立 TS 构建 → 安装 → 动态 import → 单实例 React → 真实会话收发」全链路；`apps/task-manager/` 随后按同一单元形态重写（file-explorer / file-viewer 亦同），顶层组件一律经 props 接收 agent（不再闭包捕获 ctx）。
5. 完成后再按 D1/D2/D6 决策收口 monorepo 编排与 SDK 抽取。

已明确的后续增强项（未排期）：zip/tgz 上传安装、overwrite 覆盖安装的版本迁移与安装前确认。

> 注：本评估与 [app-host-contract-onion-adaptation.md](./app-host-contract-onion-adaptation.md) 互补——那篇定「AgentChannel 契约面」，本篇定「App 单元与双端工程形态」。两者收敛后合入宿主内核设计。
