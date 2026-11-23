---
title: onion dsh 能力差距补齐——本轮变更摘要（#5 / #4 / #3 / #13）
description: 本轮按 dsh 能力差距清单逐一推进剩余工作项的变更汇总：附件体系、office 三件套、workflow 编排、对外协议三形态（ACP / SDK wire / GitHub webhook），含配套修复与验证结论。
keywords: onion, dsh 对齐, attachment, office, workflow, ACP, SDK wire, webhook, 变更摘要
author: AI 助手
date: 2026-11-18
status: done
category: reports
---

# onion dsh 能力差距补齐——本轮变更摘要

## 概述

本轮以 [onion-dsh-capability-gaps.md](../solutions/onion-dsh-capability-gaps.md) 为底稿，把剩余工作项（#5 附件体系、#4 office 三件套、#3 workflow 编排、#13 对外协议形态）逐一实施、逐一提交。共 8 个提交（`f36c716` → `e9fe2ab`），累计约 5548 行新增、113 行删除（按提交累计口径，跨提交同文件有重叠）。

实现基调：**对齐 dsh 语义、如实落地 Rust 侧形态**，凡与本仓能力边界不一致处均在提交信息与文档中「诚实降级」标注，不谎报能力。

## 提交清单

| 提交 | 工作项 | 内容 | 变更量 |
| --- | --- | --- | --- |
| `f36c716` | #5 | 附件体系：内容寻址存储 + 准入 + 上传端点 | 16 文件，+1607/-16 |
| `0fe09a8` | #4 | office 三件套：soffice headless 引擎 + convert/recalculate/render | 9 文件，+734/-1 |
| `f857424` | 修复 | onion-tui 补齐 ForkSeed 渲染分支，修复工作区编译（E0004） | 1 文件，+2 |
| `a744f1e` | #3 | workflow：声明式 JSON 编排 + 并发 fan-out + 后台 job | 16 文件，+1733/-22 |
| `4b49385` | #13-1 | ACP stdio 接入（编辑器子进程形态） | 4 文件，+567/-3 |
| `68c8d3c` | #13-2 | SDK wire stdio 协议 + 诊断回显改走 stderr | 5 文件，+533/-70 |
| `ef8c1f6` | 修复 | workflow 冒烟示例 clippy 冗余闭包 | 1 文件，+1/-1 |
| `e9fe2ab` | #13-3 | GitHub 入站 webhook（HMAC-SHA256 验签） | 6 文件，+371 |

## 分项要点

### #5 附件体系（`f36c716`）

对齐 dsh `packages/attachment` 两包（seam + local），onion 以单模块承载：

- **存储**：sha256 内容寻址对象存储（staged 写 → hard-link → 只读 → 目录 fsync），图片存 `objects/`、文件存 `file-objects/` + `files/` 别名目录，对象级去重。
- **准入**：canonical base64 + 批量限额（张数 / 聚合字节 / 单图 / 像素 / 边长 / 媒体类型），prepare-then-commit 整批原子（整批拒绝零落盘）；错误码与 dsh 同名（`TOO_MANY_IMAGES` / `INVALID_IMAGE_BASE64` / `ATTACHMENT_CORRUPT` 等）。
- **REST 接线**：`POST /v1/attachments/images` 与 `/v1/attachments/files`；消息入口 `FilePart.bytes` 激活——image/* 合并准入后回填 data URL Image 块，其余文件入库以文本块描述引用，bytes 缺省维持 uri 透传。压缩/重编码仍由既有 media 模块承担（模块文档声明取舍）。
- **测试**：store/admission 单测 13 项、dto 单测 6 项、端到端冒烟（去重 / 限额 / 错误模型 / 鉴权）。

### #4 office 三件套（`0fe09a8`）

对齐 dsh `packages/document/tool-office`。Rust 侧无嵌入式 LibreOfficeKit，改用 **soffice CLI headless**：

- 引擎定位顺序 `ONION_SOFFICE` → PATH → 常见安装路径；独占 profile 一次性 run 目录；超时 60s、输入 50MB / 产物 64MB 上限；错误码与 dsh 同名（`unavailable` / `invalid-document` / `timeout` 等）。
- 私有输出店落系统临时目录 `<tmp>/onion-office/<pid>-<n>/`（不写会话工作区），24h 懒清扫。
- 三工具：`office_convert`（同家族格式转换）、`office_recalculate`（同格式 round-trip 重算）、`office_render`（首页 PNG，入附件库 + 多模态图片块，经 media 预算限幅）。
- 已注册进 coding / work 预设。诚实降级：render 仅首页/首表/首屏（CLI 无逐页渲染），schema 不谎报 pages/dpi 能力。

### #3 workflow 编排（`a744f1e`）

对齐 dsh `packages/workflow/tool-workflow`。**核心取舍（诚实降级）**：dsh 接收 JS 脚本（agent/pipeline/parallel/phase/log 钩子）在沙箱 JS 运行时执行；onion 不引入 JS 运行时，落地为**声明式 JSON 程序**。

- [program.rs](../../crates/onion/workflow/program.rs)：fail-closed 解析校验（meta.name/description 必填、step id 唯一、显式 phase 须在 meta.phases、for_each 须为 args 数组路径、concurrency 缺省 4 上限 16）；模板替换 `{{args.*}}` / `{{steps.<id>.*}}` / `{{item}}` / `{{index}}`，未解析即报错不静默留空。
- [runner.rs](../../crates/onion/workflow/runner.rs)：逐步骤经 `SubagentRuntime::run_round` 派发全新子代理（depth=1）；for_each 信号量限流并发，结果按输入下标装配；单 agent 失败解析为 null（不终止编排，对齐 dsh）；终态三态 completed/cancelled/error；进度回调按 phase/step 粒度产出。
- [tools/workflow.rs](../../crates/onion/tools/workflow.rs)：`workflow` 工具（program/args/run_in_background）；前台非 completed 记 is_error，结果上限 50000 字符；后台经 JobRegistry 注册为 workflow job，`job_output` 读进度、`job_kill` 经新增取消钩子取消。
- **onion-jobs 配套**：新增 `JobKind::Workflow` 与 `register_with_cancel` 取消钩子（回调在释放内部锁后触发，允许重入）。
- **接线**：assemble 注册工具 + `tool:workflow` 系统提示段（order 2750，明示「不是 JS 脚本」）；coding / work 预设工具面加入。
- **测试**：program/runner 单测（哑 LLM）+ 工具层单测 + `examples/workflow_real_smoke.rs` 真实 API 冒烟（2 项 fan-out + 1 汇总 = 3 子代理，PASS）。

### #13 对外协议三形态（`4b49385` / `68c8d3c` / `e9fe2ab`）

三者与既有 A2A / REST 共用同一轮次驱动入口（`gate.enqueue` + `inbox().followup` + `spawn_turn`），仅 wire 语义不同。

**#13-1 ACP（`4b49385`）**——对齐 dsh `packages/acp` 编辑器子进程形态：`--stdio acp`（或 `ONION_SERVER_STDIO=acp`），NDJSON JSON-RPC 2.0 over stdin/stdout，不起 HTTP。实现 `initialize` / `authenticate` / `session/new` / `session/prompt`（期间 `session/update` 流式回推 message/thought/tool_call 块，结束返回 `stopReason`）/ `session/cancel`；写任务独占 stdout（mpsc 串行化），取消可插队；首行剥 BOM。诚实降级：无 `session/load`、不注入 MCP、跳过非文本内容块。8 单测 + 真实 stdio 冒烟 PASS。

**#13-2 SDK wire（`68c8d3c`）**——对齐 dsh `packages/sdk/protocol`：`--stdio sdk`。与 ACP 的语义差别：`session/prompt` 立即返回**入队回执** `{messageId}`，进展经 `session.event` 通知按落盘顺序流式回推（与 REST `/v1/sessions/<id>/events` 同源）+ `session.status`（running/idle）。实现 `initialize` / `session/prompt` / `session/wait` / `session/working-directory/get` / `shutdown`；每会话一个事件泵按 durable 快照增量回推（锁内只做快照，发送在锁外）；未知 sessionId 按连接 cwd 懒建。诚实降级：`provider`/`model` 等参数接受但不改变本进程路由；`session/query`、`session/resume`、`user-question/ask` 回推未实现。4 单测 + 真实 stdio 冒烟（20 帧 event）PASS。

**#13-3 GitHub webhook（`e9fe2ab`）**——对齐 dsh `packages/webhook/webhook-github`：`POST /webhooks/github`，`X-Hub-Signature-256`（HMAC-SHA256）常量时间验签（sha2 手写 RFC 2104，不引新依赖，RFC 4231 已知答案测试）；密钥只经 `ONION_GITHUB_WEBHOOK_SECRET` 注入，空 = 端点关闭（503 fail-closed）。settled 规则：仅 issues / pull_request 的 `opened` 派发（提示词按字符截断 8000），其余 202 接收但 IGNORE；`X-GitHub-Delivery` 512 条窗口去重防重投；body 上限 1 MiB；本面不认运维令牌，鉴权由事件源签名承担；`ONION_GITHUB_WEBHOOK_CWD` 指定会话权限根。诚实降级：仅 GitHub 单事件源、不做 PR 增量触发、不回写 check run/comment。5 单测 + 真实 HTTP 冒烟（签名 / 重投 / 未签名 / ping / ignore 全覆盖）PASS。

## 横切修复与配套

- **onion-tui 编译修复（`f857424`）**：w9 引入 `SessionEvent::ForkSeed` 时漏了 TUI 转录穷尽匹配（E0004）；按约定归为 log-only 分支（fork 前缀已作为普通消息重放，标记只承载血缘元信息）。
- **诊断回显 stdout → stderr（`68c8d3c` 同提交）**：`EchoObserver` / `Gatekeeper` / `ApprovalGate` 的 `[inbox] inserted …` 等诊断原走 stdout，在 ACP/SDK stdio 模式下实测污染协议流；全部改走 stderr，CLI 前端终端仍可见。
- **clippy 收尾（`ef8c1f6`）**：workspace deny `redundant_closure_for_method_calls` 下冒烟示例的冗余闭包（仅 `--all-targets` 可见）。
- **onion-jobs 取消钩子**：`register_with_cancel` 让非进程型执行器（workflow）的 kill 走同一控制面。

## 验证结论

| 项 | 结果 |
| --- | --- |
| onion 全量回归 | 1020 项通过（跳过 2 个环境既有失败项 `real_pty_roundtrip` / `spawn_plan_pwsh_uses_encoded_command`） |
| onion-server 回归 | 82 项全绿 |
| clippy（-p onion / -p onion-jobs / -p onion-server --all-targets） | 无新增告警 |
| `cargo check --workspace --all-targets` | 仅剩 4 个**既有** warning（jsonrpc.rs / rest/error.rs，非本轮引入） |
| 真实冒烟 | workflow 编排 / ACP stdio / SDK wire stdio / GitHub webhook HTTP，四条链路均走 local 供应商实测通过 |
| 工作区 | 全部提交后 `git status` 干净（commands.rs 按约定不提交，已与 HEAD 一致） |

## 影响面与后续

- **新增能力面**：`workflow` 工具进入 coding/work 预设；`onion-server` 新增 `--stdio acp|sdk` 两种 stdio 形态与 `/webhooks/github` 端点；`/v1/attachments/*` 上传端点与 office 三工具。
- **行为变化**：诊断输出统一改 stderr——依赖 onion CLI stdout 做管道解析的下游需改读 stderr。
- **后续候选**（本轮明确不做，已在提交信息声明）：JS 脚本编排、ACP `session/load`、SDK `session/query`/`session/resume`、webhook 多事件源与回写能力、office 逐页渲染。
