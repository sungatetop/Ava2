---
title: 边车生命周期设计
description: ava2 边车 onion-server 的进程模型、启动装配、双端基址解析、健康发现、App 仓库与数据目录约定，以及与宿主（web-ui / 未来桌面端）的集成契约。
keywords: onion-server, 边车, 生命周期, App 仓库, state.json, 基址解析, 健康检查
author: AI 助手
date: 2026-11-08
updated: 2026-11-08
status: done
category: design
---

# 边车生命周期设计

onion-server（`crates/onion-server`）是 ava2 的**常驻边车**：axum HTTP/WS/SSE 单进程，承载 A2A v1.0 agent 端点、`/v1` RESTful 运维面与 App 仓库。宿主（web-ui 浏览器形态、未来的桌面端）只经 HTTP/WS 消费，**不依赖边车本机文件系统能力**。

## 1. 进程模型

| 形态 | 启动方 | 说明 |
| --- | --- | --- |
| web-ui 开发态 | 开发者手动/脚本（`onion-server.exe --port 9800`） | 需配置 `ONION_SERVER_CORS_ORIGINS` 包含 web-ui 源 |
| web-ui 纯浏览器 | 同上（远程或本机） | `?backend=` 指向可达地址 |
| 桌面端（已落地） | Tauri 壳在 setup 阶段 spawn 子进程（`src-tauri/src/sidecar.rs`，`dev.ps1` 一键启动） | 参照 ava 的 sidecar 约定：注入端口/CORS 环境变量、窗口销毁 kill 子进程树、日志滚动（见 §7） |

生命周期极简：**启动即常驻，ctrl-c（SIGINT）优雅关停**（进行中请求走完），无守护/重启逻辑——崩溃恢复交由宿主重启进程（桌面端壳负责，web 态交开发者/运维）。

## 2. 启动装配（main.rs）

按序装配，任一硬失败 `exit(1)`：

1. **端口**：`--port N` > `ONION_SERVER_PORT` > 默认 `9800`；绑定 `127.0.0.1:port`（仅回环，不对外网暴露）。占用即 `bind failed` 退出——**宿主启动前应先探测端口或显式传空闲端口**。
2. **模型配置**：`Config::from_env()`（`SMOKE_LLM_*` 系列环境变量；打印 `model / base_url`）。
3. **日志目录**：`<data_home>/logs/onion-server/`（创建失败仅告警，不阻断）。
4. **工作区兜底**：`ONION_SERVER_WORKSPACE` > 进程 cwd；决定会话默认落盘容器 `<ONION_HOME>/sessions/<normalized-cwd>/`（语义对齐 dsh `sandboxPolicy.workspaceRoot`：会话未显式给 `cwd` 时用它）。
5. **访问令牌**：`ONION_SERVER_TOKEN` 非空时 `/v1` 要求 `X-Onion-Token`（运维面）；`ONION_A2A_TOKEN` 非空时 `/a2a*` 接受 mesh 令牌或运维令牌任一（agent 面）。
6. **AgentCard**：well-known 发现文档（`/.well-known/agent-card.json`），含 instance_id、capabilities、skills。
7. **App 仓库**：`ONION_APPS_DIR` > `<data_home>/apps`，创建失败 exit(1)（App 面是宿主必需能力）。
8. **CORS**：`ONION_SERVER_CORS_ORIGINS`（逗号分隔）配置时启用；未配置不启用（同源形态无需）。
9. **A2A 对端表**：`ONION_A2A_PEERS` 指向 JSON（`[{"name","url"}]`）；缺失或解析失败一律空表（不兜底、不降级）。

## 3. 双端基址解析（env.ts，宿主侧）

```
桌面端：Tauri invoke("backend_base")  ──┐
                                        ├──→ env.resolveBackendBase()（进程内缓存一次）
浏览器：URL ?backend=http://host:port ──┤
                                        │
缺省：  http://127.0.0.1:9800        ───┘
```

- 解析结果同时喂给 **Agent 通道**（REST + WS）与 **App 仓库客户端**——App 侧禁止自行判断运行环境。
- 基址是 boot 期一次性快照：运行中改 `?backend=` 需刷新页面。

## 4. 健康与发现

| 端点 | 用途 | 宿主行为 |
| --- | --- | --- |
| `GET /v1/health` | 存活探测 | loader boot 时探测一次；**失败不阻断装配**（App 可本地装配），仅落标题栏状态点 |
| `GET /.well-known/agent-card.json` | A2A 发现文档 | 供 mesh / 调试；宿主 boot 不消费 |
| `GET /v1/models` / `GET /v1/agents` | 能力枚举 | App 经 `ctx.agent.listModels/listAgents` 惰性调用 |

标题栏状态点三态：`ok`（probe 通过）/ `bad`（probe 失败但装配完成）/ `idle`（boot 中）。

## 5. App 仓库（rest/apps.rs）

安装模型对齐 dsh 插件：**一个 App = 一个目录，安装即受控复制**。

### 5.1 目录与登记表

```
<apps_dir>/                       # ONION_APPS_DIR 或 <data_home>/apps
├── state.json                    # 登记表：{ apps: { <id>: { enabled, installed_at_ms } } }
└── <id>/                         # 安装产物（受控复制，跳过 node_modules / .git）
    ├── manifest.json
    └── ...（ESM 产物 + 资源）
```

- **未登记的目录不会被宿主发现**（登记表是唯一权威）。
- `state.json` 原子写（tmp + rename），避免半写损坏；启停态持久化，重启保持。
- 卸载 = 删目录 + 删登记项（404 幂等）。

### 5.2 端点与限额

| 端点 | 限额 / 校验 |
| --- | --- |
| `POST /v1/apps/install {path, overwrite}` | 已装未 overwrite → 409；只接受预构建产物（**安装期不执行任何构建脚本**）；复制跳过 `node_modules/.git` |
| `DELETE /v1/apps/{id}` | id 校验：`[A-Za-z0-9._-]` 单段、≤64、不为 `.`/`..`（防路径逃逸） |
| `PUT /v1/apps/{id}/enabled {enabled}` | 幂等 |
| `GET /v1/apps` | `{apps: [{manifest, enabled, installed_at_ms}]}` |
| `GET /v1/apps/{id}/{*path}` | 静态产物；单文件 ≤16 MiB；`manifest.json` ≤64 KiB |

### 5.3 hostApi 兼容

边车 `HOST_API_SUPPORTED = 2`，与宿主 `types.ts` 常量保持一致；清单 `hostApi` 超限在宿主 parseManifest 阶段判 failed（边车不做语义校验，只做仓库与静态服务）。

## 6. 数据目录约定

| 路径 | 内容 |
| --- | --- |
| `<ONION_HOME>/sessions/<normalized-cwd>/<sessionId>/` | 会话 transcript（边车自持） |
| `<ONION_HOME>/logs/onion-server/` | 边车日志 |
| `<ONION_HOME>/apps/` | App 仓库（§5） |

`ONION_HOME` 由 onion 的 `paths::data_home()` 解析（环境变量覆盖，缺省用户主目录）；宿主与 App **不直接读写**这些目录，一律经 REST。

## 7. 与桌面端的集成契约（已落地，参照 ava sidecar 约定）

1. 壳在 `setup` 阶段 spawn 边车子进程：`onion-server.exe --port <动态分配>`（或 `AVA`→`ONION` 环境变量注入），**避免固定端口冲突**。
2. 日志重定向到 `<data_dir>/logs/`，每次启动滚动 `.old`。
3. 窗口销毁时 kill 子进程（防残留占用端口）。
4. 前端经 `invoke("backend_base")` 取实际基址（§3 第一优先级）。
5. 壳自带健康重试：spawn 后轮询 `/v1/health` 直到 ok 或超时报错。

## 8. 已知边界

- 边车无鉴权默认开放（`ONION_SERVER_TOKEN` 未配置时）：仅绑定回环是主要防线；远程部署必须配 token。
- 端口占用 → 直接 exit(1)，宿主侧需自行选择空闲端口（桌面端壳动态分配；web 态由启动脚本负责）。
- App 仓库不做版本管理：overwrite 安装直接替换目录（版本回滚靠重新安装旧产物）。
- CORS 允许方法/头为 Any（开发态权衡）；收紧是后续安全项。
