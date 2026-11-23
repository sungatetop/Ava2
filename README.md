# ava2

Tauri 桌面壳 + 可插拔 App 契约（AppHost）+ onion-server Agent 运行时边车。

ava2 把「一切皆 App」落到工程上：宿主壳只负责窗口、边车生命周期与端口注出，Agent 能力由
`onion-server` 边车以 HTTP / WS / SSE 提供，界面由一组遵循统一契约的 App 动态装配。

## 架构

```
┌──────────────────────────── Tauri 壳（src-tauri）────────────────────────────┐
│  窗口 / 边车拉起与守护 / 端口注出（不含 Agent 逻辑）                          │
│                                                                              │
│  ┌──────────── 前端 AppHost（src）────────────┐   ┌── onion-server 边车 ──┐  │
│  │ slot 注册表 + 运行时动态 import            │──▶│ HTTP / WS / SSE        │  │
│  │ import map 单实例 React                    │   │ Agent 运行时内核       │  │
│  │ App：setup(ctx) → slots.inject             │   └────────────────────────┘  │
│  └────────────────────────────────────────────┘                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

- **壳（`src-tauri`）**：窗口、边车生命周期、端口注出；不编译依赖任何 onion crate，只在运行时 spawn 边车。
- **Agent 运行时（`crates/onion-server`）**：边车二进制，前端经 `http://127.0.0.1:<port>` 消费。
- **前端（`src`）**：AppHost 宿主，slot 注册表 + 运行时动态 import + import map 单实例 React。
- **App（`apps/*`）**：预置与三方同构，统一 `setup(ctx)` → `slots.inject` 路径。

## 目录结构

```
ava2/
├── Cargo.toml          # Rust workspace（onion 系 crate）
├── dev.ps1             # 一键调试：构建边车 → 拷贝 resources → tauri dev
├── crates/             # onion 内核 / 边车 / CLI / TUI / 沙箱 / SDK
├── src/                # 前端 AppHost（React 19 + Vite + TS）
├── src-tauri/          # Tauri 壳（独立 Cargo.lock，不并入 workspace）
├── apps/               # 可插拔 App（chatapp / file-viewer / skills-manager ...）
├── packages/app-sdk/   # App 开发 SDK 类型
├── skills/             # ava2-app-dev 技能（App 开发指南与校验脚本）
└── docs/               # 架构设计与开发指南（入口 docs/index.md）
```

## 环境要求

- Node.js 20+ 与 npm
- Rust stable（`cargo`）
- Windows 下需 WebView2（Tauri 2 依赖）

## 快速开始

```powershell
npm install
.\dev.ps1
```

`dev.ps1` 会依次：`cargo build -p onion-server -p onion-sandbox` → 拷贝产物到
`src-tauri/resources/` → 设置 `ONION_HOME` / `ONION_SERVER_PORT` → `npm run tauri dev`。

常用参数：

```powershell
.\dev.ps1 -SkipBuild      # 复用已有边车产物
.\dev.ps1 -Port 9801      # 指定边车端口（默认 9800）
```

开发数据根固定为仓库内 `.onion-test`（已 gitignore），与生产默认 `~/.onion` 隔离。

## 模型配置

默认模型路由从以下来源解析（优先级从高到低）：

1. 环境变量 `SMOKE_LLM_BASE_URL` / `SMOKE_LLM_API_KEY` / `SMOKE_LLM_MODEL`
2. `<ONION_HOME>/local-defaults.json`（本地文件，不入库）：

```json
{ "baseUrl": "http://127.0.0.1:8080/v1", "apiKey": "sk-..." }
```

3. 空值（未配置，需在应用内「模型管理」中配置供应商）

## 构建

```powershell
npm run build          # 前端（vendor + tsc + vite build）
cargo build --release  # 边车与内核
npm run bundle         # 桌面安装包（scripts/publish.ps1，需本地签名密钥）
```

## 测试

```powershell
cargo test -p onion
cargo test -p onion-server
npm run check:contracts   # App 契约一致性校验
```

## 文档

- [docs/index.md](docs/index.md) — 文档总入口与阅读顺序
- [docs/references/app-dev-standards.md](docs/references/app-dev-standards.md) — App 开发规范总纲
- [docs/design/DESIGN.md](docs/design/DESIGN.md) — AppHost 设计系统（`--ava-*` 令牌）

## 贡献

见 [CONTRIBUTING.md](CONTRIBUTING.md) 与 [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)。

## 许可证

[MIT](LICENSE)
