# ava2 项目开发规范

本文件是 **ava2 独立工程**的项目规则（与根目录 `.trae/rules/project_rules.md` 的 ava 平台规范并行，冲突时以本文件为准）。所有 AI 助手与开发者在本目录内工作均须遵守。

## 1. 项目定位

ava2 是 **AppHost 原型的落地实践**：Tauri 桌面壳 + 可插拔 App 契约（一切皆 App）。

- **壳（`src-tauri`）**：只负责窗口、边车生命周期、端口注出，不含 Agent 逻辑。
- **Agent 运行时**：`onion-server` 边车（HTTP / WS / SSE），前端经 `http://127.0.0.1:<port>` 消费。
- **前端（`src`）**：AppHost 宿主（slot 注册表 + 运行时动态 import + import map 单实例 React）。
- **App**：预置与三方同构，统一 `setup(ctx)` → `slots.inject` 路径。

## 2. 目录结构

```
ava2/
├── Cargo.toml              # Rust workspace（仅 onion 系 crate）
├── clippy.toml             # clippy 配置（与 ava 一致）
├── rustfmt.toml            # rustfmt 配置
├── dev.ps1                 # 一键调试：构建边车 → 拷贝 resources → tauri dev
├── crates/                 # Rust workspace 成员（onion 系，从 ava/crates 复制）
│   ├── onion/              # 内核库
│   ├── onion-jobs/         # 后台任务注册表
│   ├── onion-cli/          # CLI 入口
│   ├── onion-tui/          # TUI 入口
│   ├── onion-sandbox/      # OS 级沙箱执行器
│   ├── onion-sdk-server/   # SDK 服务端
│   ├── onion-sdk-client/   # SDK 客户端
│   └── onion-server/       # Agent 运行时边车（HTTP/WS/SSE）
├── src/                    # 前端（React 19 + Vite + TS）
├── src-tauri/              # Tauri 壳（独立 Cargo.lock，不并入 workspace）
│   ├── src/sidecar.rs      # onion-server 边车拉起/守护
│   └── resources/          # 边车产物（构建后拷贝，不入库）
└── docs/                   # 文档根（见第 3 节）
```

## 3. 文档规范

### 3.1 保存位置

- 所有文档 / 报告**必须以 Markdown（`.md`）保存**，统一存放于 **`ava2/docs/`**。
- 禁止散落在项目根目录、`src/`、`crates/` 或其他非文档目录。

### 3.2 分类归档

`ava2/docs/` 采用六分类结构，根部放 `index.md` 作为入口：

```
ava2/docs/
├── index.md          # 文档入口（目录导航 + 阅读顺序）
├── design/           # 设计：架构总览 / 模块设计 / 契约设计
├── decisions/        # 决策：ADR 与历史决策记录
├── plans/            # 计划：roadmap / 后续计划 / backlog
├── solutions/        # 解决方案：具体功能/问题的实现方案
├── reports/          # 报告：进度快照 + 评审/分析/验收报告
└── references/       # 参考资料：使用指南 / 外部框架研究 / 源码地图
```

分类判定：设计 vs 解决方案看是否针对具体功能/问题（后者进 `solutions/`）；计划只放宏观 roadmap；报告统一收纳进度与评审类。

### 3.3 Frontmatter

所有文档头部必须使用 YAML Frontmatter（位于文件第一行，上方不得有空行）：

```markdown
---
title: 文档标题
description: 一句话说明核心内容、用途与受众。
keywords: 关键词1, 关键词2, 关键词3
author: AI 助手
date: 2026-11-07
updated: 2026-11-07
status: draft
category: design
---

# 文档标题
```

必填：`title` / `description` / `keywords` / `date`；可选：`author` / `updated` / `status`（`draft`/`review`/`done`/`archived`）/ `category`（对应六分类子夹）。

## 4. Rust 开发规范

### 4.1 构建与测试

- workspace 在 `ava2/` 下：`cargo build` / `cargo test`；改动单个 crate 优先 `cargo test -p <crate>`。
- 不使用 `cargo test --all-features` 做常规验证（扩大构建矩阵与磁盘占用）。
- 修改依赖后确认 `Cargo.lock` 随改动提交。
- 编译耗时属正常现象，不要用 PID 强杀。

### 4.2 代码风格

- 沿用 `[workspace.lints.clippy]` 的 deny 规则集（`uninlined_format_args` / `collapsible_if` / `unwrap_used` / `expect_used` 等），由 clippy 强制。
- `format!` 能内联的变量直接内联；折叠可折叠的 `if`；优先方法引用而非闭包；`match` 尽量穷尽。
- 避免 bool / 含义含糊的 `Option` 位置参数；无法改 API 时按 `argument_comment_lint` 加 `/*param_name*/` 注释。
- 新增 trait 须带 doc 注释；trait 内异步方法用原生 RPITIT 显式声明 `Send` future，禁用 `#[async_trait]`。
- 模块目标 <500 LoC（不含测试），超约 800 LoC 应拆分；优先私有模块 + 显式导出公共 API。
- 单元测试一律使用独立 `*_tests.rs` 兄弟文件（`#[path]` 挂载），禁止内联 `#[cfg(test)] mod tests`。
- 跨平台：除非功能明确与 OS 绑定，测试与特性须同时支持 Linux / macOS / Windows。

### 4.3 分层约束

- `onion` 是 Agent 运行时内核库，**不依赖 tauri**。
- `onion-server` 是边车二进制，仅依赖 `onion` / `onion-jobs` 与 HTTP 栈。
- `src-tauri` 不编译依赖任何 onion crate，只**运行时 spawn** `onion-server` 二进制。
- 数据根统一经 `onion::paths::data_home()` 解析（`ONION_HOME` > `~/.onion`），禁止各模块自行拼接。

## 5. 前端开发规范

- 技术栈：React 19 + Vite + TypeScript；函数组件 + Hooks，禁止类组件。
- 目录按功能域组织：`src/host/`（AppHost 内核）、`src/components/<domain>/`、`src/api/`、`src/types/`、`src/hooks/`、`src/store/`。
- 类型按领域拆分放 `src/types/`，禁止堆在根 `types.ts`；请求脚本按领域拆分放 `src/api/`，只有 `client.ts` 允许直接使用 `fetch`。
- 样式使用 CSS 变量 / design tokens，禁止硬编码色值字号；新增样式须同时考虑亮/暗主题。
- **单实例 React 不变式**：host alias、JSX 运行时 shim、import map 三方必须解析到同一 URL（vendor 产物放 `src/vendor/`，不放 `public/`），否则 App 组件 hooks 崩溃。
- App 契约警示：`setup` / 组件工厂只返回组件函数，不返回已渲染 element；`events.on` 的 Disposable 在 hooks 中必须以 `() => d.dispose()` 作为 cleanup 返回。

## 6. 开发调试

### 6.1 数据目录

- 开发调试统一将 `ONION_HOME` 指向仓库内 `.onion-test`（已 gitignore），与生产默认 `~/.onion` 隔离。
- 禁止在代码中硬编码该路径；统一经 `onion::paths::data_home()` 解析。

### 6.2 启动方式

- 统一使用 `ava2/dev.ps1`（在 `ava2/` 下执行 `.\dev.ps1`），**不要**直接 `npm run dev` 单独起 Vite。
- 脚本流程：`cargo build -p onion-server` → 拷贝到 `src-tauri/resources/` → 设置 `ONION_HOME` / `ONION_SERVER_PORT` → `npm run tauri dev`。
- 参数：`-SkipBuild`（复用已有产物）、`-Port <n>`（默认 9800）。
- 边车由 Tauri 壳在 `setup` 阶段 spawn（`src-tauri/src/sidecar.rs`），日志落 `<ONION_HOME>/logs/onion-server.log`（每次启动滚动为 `.old`），窗口退出时终止进程树。

### 6.3 热重载与重启

- **仅改前端**（`src/**`）：Vite 热重载，无需重启；前端改动须在浏览器交互验证。
- **改 onion / onion-server 等 Rust 边车**：需重启 `dev.ps1`（先结束占用端口的旧进程）。
- 端口冲突排查：`Get-NetTCPConnection -LocalPort 9800 -State Listen | Select-Object OwningProcess`，必要时 `.\dev.ps1 -Port 9801`。

## 7. 测试与验证

- 测试使用**真实 API 调用**，不得使用模拟数据。
- 冒烟测试必须截图留证。
- 边车冒烟基线：`GET /v1/health` 返回 `{"status":"ok",...}`；`GET /v1/meta` 返回工作区与会话落盘位置。
