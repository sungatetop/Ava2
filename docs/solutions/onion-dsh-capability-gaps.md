---
title: onion 与 dsh 能力差距清单（五契约面之外扩展扫描）
description: 在已收尾的 32 项交互契约对齐之外，对 ava2 onion 与 deepseek-harness 做四域扩展扫描（工具面+沙箱 / 技能+MCP+hooks+上下文 / 会话+存储+服务面 / 内核循环+LLM+compaction），产出按影响分级排序的未对齐清单与修复优先级建议。
keywords: onion, dsh, deepseek-harness, 能力差距, 沙箱, 工具面, MCP, hooks, 会话持久化, compaction, claim 语义
author: AI 助手
date: 2026-11-17
status: review
category: solutions
---

# onion 与 dsh 能力差距清单（五契约面之外扩展扫描）

## 背景

`onion-dsh-interaction-gaps.md`（32 项，已收尾）覆盖了五个交互契约面：斜杠命令、subagent 委派、plan/goal/compact、审批与 ask_user、会话事件模型。本文是对这五个契约面**之外**的能力域做的一次系统扩展扫描结论，按四路并行调研汇总：

1. 内置工具面 + 执行沙箱
2. 技能 + MCP + hooks + 上下文注入
3. 会话持久化 + 查询 + 后台任务 + 存储/凭据 + 对外服务面
4. 内核轮次循环 + LLM 适配层 + compaction 细节

分级口径：**P0** = 运行时故障/安全阻断；**P1** = 能力缺失但可绕过；**P2** = 边界/风格/打磨差异；**P3** = dsh 专有或深绑定能力，建议搁置。

统计：P0 × 1，P1 × 17，P2 × 15，P3 × 7。

## 总览

| # | 级 | 域 | 差距点 |
|---|---|---|---|
| 1 | P0 | 沙箱 | OS 沙箱平台覆盖 + 未配置时 shell 直通 |
| 2 | P1 | 工具 | apply_patch 多文件补丁工具缺失 |
| 3 | P1 | 工具 | workflow 编排工具缺失 |
| 4 | P1 | 工具 | office 三件（render/convert/recalculate）缺失 |
| 5 | P1 | 工具 | attachment 体系缺失 |
| 6 | P1 | MCP | MCP 无运行期生命周期/管理面（含工具 re-sync） |
| 7 | P1 | hooks | hooks 仅支持 App contributes 来源 |
| 8 | P1 | 技能 | skill 发现缺 project 级 root 与 preset 挂载层 |
| 9 | P1 | 会话 | 会话 fork/分支缺失 |
| 10 | P1 | 查询 | 查询过滤面差距（FTS/时间/seq/cursor） |
| 11 | P1 | 持久化 | 崩溃恢复与写完整性（torn-tail/写租约/压缩） |
| 12 | P1 | 凭据 | 凭据子系统（含 `${credential:…}` 模板） |
| 13 | P1 | 服务面 | 对外协议形态（ACP / SDK wire / webhook 入站） |
| 14 | P1 | spill | 通用 spill 工件库缺失 |
| 15 | P1 | 内核 | claim 语义：同批消息拆多个 turn（已知残留，复核确认） |
| 16 | P1 | LLM | LLM 错误分类学 + 退避重试策略 |
| 17 | P1 | compaction | per-model 压缩策略 |
| 18 | P2 | 工具 | PTC（programmatic tool calling） |
| 19 | P2 | 工具 | web_search provider 面 |
| 20 | P2 | 工具 | web_fetch 渲染质量 |
| 21 | P2 | 工具 | persistent bash 变体 |
| 22 | P2 | guard | 横切设施：repeat-tool-reminder 阈值 + per-tool 超时预算 |
| 23 | P2 | 工具 | schedule 面差（缺 view） |
| 24 | P2 | hooks | hooks-codex 方言缺失 |
| 25 | P2 | hooks | hooks 触发点覆盖 12/30 |
| 26 | P2 | settings | settings 通用 schema/forms/redact 面 |
| 27 | P2 | compaction | image offload 持久化 |
| 28 | P2 | goal | checkpoint race 栅栏 |
| 29 | P2 | 内核 | max-tokens sticky 终态 |
| 30 | P2 | jobs | 输出反馈粒度（spill 路径 + gapBefore） |
| 31 | P2 | 遥测 | 遥测/匿名身份完全缺失 |
| 32 | P2 | 查询 | 查询授权口径（live/persisted 双源） |
| 33 | P3 | — | tmux/harness-dev/time request-zone 上下文族 |
| 34 | P3 | LLM | prompt caching（cacheWrite 计量 + 前缀 digest 绑定） |
| 35 | P3 | LLM | DeepSeek files API + 多 provider 目录 + token 计价 |
| 36 | P3 | context | context 注入管道族（time/@file/session-reference） |
| 37 | P3 | 专有 | dsh 深绑定能力（deepseek-account/authorization/host 插件体系） |
| 38 | P3 | 说明 | onion 演示 listener 为 ava2 独有 |

## P0：运行时故障/安全阻断

### 1. OS 沙箱平台覆盖 + 未配置时 shell 直通

- **dsh**：三平台 OS 级强制且 fail-closed——Linux=bwrap→Landlock 链、macOS=Seatbelt、Windows=ACL 受限 token；confinement 缺失或不可用时**拒绝执行而非透传原 argv**（`packages/sandbox/sandbox-local/src/index.ts:2-6`）。
- **onion**：`onion-sandbox` 仅实现 Windows（受限 token + ACL + Job Object），Linux/macOS 显式报「未实现」（`crates/onion-sandbox/src/platform/mod.rs:17-25`）；且未配置沙箱 exe 时 shell **直通无隔离**（`crates/onion/assemble.rs:614-619` 自注「否则直通（无 OS 强制）」）。
- **影响**：ava2 当前生产环境即 Windows 时可用，但安全下限依赖「内容分类器正确性」而非 OS 强制；跨平台分发后安全模型不成立。
- **修复方向**：Linux bwrap/Landlock、macOS Seatbelt 落地；或至少把「未配置沙箱」改为显式审批兜底（fail-closed 化）。

## P1：能力缺失（可绕过）

### 工具面

**2. apply_patch 多文件补丁工具**（dsh `packages/fs/tool-fs/src/apply-patch.ts:26-38`；onion `crates/onion/tools/mod.rs:4-18` 无定义）——Add/Update/Move/Delete File 一次提交、先全量校验后顺序应用；onion 只能多次 edit 绕过。

**3. workflow 编排工具**（dsh `packages/workflow/tool-workflow/src/index.ts:318-345`；onion 仅 ralph 循环 `crates/onion/ralph/mod.rs:3`）——JS 脚本 fan-out 多 subagent、分阶段、结构化结果、后台 run + job。

**4. office 三件**（dsh `packages/document/tool-office/src/index.ts:84-96,163-206`；onion 无对应）——office_render（文档渲染成图）/office_convert（格式转换）/office_recalculate（工作簿重算）；dsh 借 fs 读权威 + 私有输出店不写工作区，补齐时须注意该语义。

**5. attachment 体系**（dsh `packages/attachment/attachment/src/index.ts:1-5` + `attachment-local/src/image.ts`；onion 全仓 grep "attachment" 0 命中）——durable 附件存储 seam + 准入 + 压缩/重编码 + 请求投影（图片变体）。

### 技能 / MCP / hooks

**6. MCP 无运行期生命周期/管理面**（dsh mcp-manager 完整管理面；onion 仅装配期一次性静态注册 `crates/onion-server/src/session.rs:796-807`）——server 死亡、配置变更、tools/list 变更后工具面静默失效；无 probe/restart/reconcile/re-sync。长会话内新增或变更 MCP 工具不感知。

**7. hooks 仅支持 App contributes 来源**（dsh settings `hooks` 支持用户级/工作区级配置；onion 唯一接线点 `crates/onion-server/src/session.rs:1615-1627`）。

**8. skill 发现缺 project 级 root 与 preset 挂载技能层**（dsh 多 root rank 100–600 + chokidar watcher；onion 仅 user 根 rank=400 无 watcher，`crates/onion/skill/filesystem.rs:8-11`；registry.rs:2 自述单层 global）。

### 会话 / 存储

**9. 会话 fork/分支缺失**（dsh `session-persistence-jsonl/src/storage.ts:73-74,106-109` 携带继承前缀 `inheritedEventCount` + v3→v4 fork 迁移；onion 仅 agent_team 的 forkProvider 且 service 自注未完成，`crates/onion/agent_team/service.rs:235-237`；查询面明示无 lineage，`crates/onion/session_query/mod.rs:8`）。

**10. 查询过滤面差距**（dsh `session-query/src/filters.ts:59,81,128,142` 的 availability/seq/created-at/time 过滤 + parentSessionIds 血缘 + SQLite FTS5 派生索引 `session-query-sqlite/src/schema.ts:127,158`；onion 小写子串匹配、无 FTS/时间/seq/cursor，`crates/onion/session_query/mod.rs:97`）。子代理日志可检索已覆盖（此前修复有效）。

**11. 崩溃恢复与写完整性**（dsh `storage.ts:49-50,70-72` torn-tail 截断修复、`:57-58` 跨进程写租约、`session-checkpoint-policy/src/index.ts:21-23` fail-closed 检查点门控、zstd 压缩 + generation 编址 + v0..v4 迁移；onion 损坏行仅跳过不截尾 `crates/onion/session.rs:510`，无写锁/压缩/格式迁移）。

### 凭据

**12. 凭据子系统**（dsh `credentials/credentials/src/index.ts:3-8` 引用式 seam：env 名引用、按次解析 + credentials-local/authorization 包；onion `credentials_file` 仅在 `crates/onion/paths.rs:38` 定义零调用，api_key 明文进 providers.json `provider_catalog.rs:58,100`）。相关子点：MCP env/headers 缺 `${credential:…}` 凭据模板，只能明文写 mcp.json。

### 服务面

**13. 对外协议形态**（dsh 有而 onion-server 缺）：ACP stdio JSON-RPC（`acp/acp/src/index.ts:1-9`）、SDK wire 协议 NDJSON stdio（`sdk/protocol/package.json:3` + client/server）、GitHub webhook 入站建会话（签名适配器 + settled rule，`webhook/lib/index.js:8`）。

### spill

**14. 通用 spill 工件库**（dsh `spill/spill/src/types.ts:32-39,76-80`：超大工具结果持久化为 SpillRef——不透明 locator + 检索提示 + fork 继承，启动清扫 GC `spill-local/src/cleanup.ts:8-15`；onion 仅 job 输出 spill 到 `logs/jobs/` `paths.rs:52-56` 与图片卸载占位 `compaction/prune.rs:60`，无通用 store、无 GC）。

### 内核 / LLM / compaction

**15. claim 语义（已知残留，本次复核确认）**

- dsh：`packages/subagent/subagent/src/manager.ts:1134` 每条 settlement 独立按父会话实时状态分类（idle→queue / 运行→steer）；`packages/core/agent-loop/src/agent.ts:171-182` send() + wakeDriver（约 :249-256）在**第一条 wake 时同步**把 phase 置 running → 同批后续消息转 steer 进 next-step；`packages/core/agent-loop/src/inbox.ts:108-113` claim() 领取**全部 next-step + 1 条 next-turn** 并入本 turn。
- onion：`crates/onion/inbox.rs:139-146` `claim_open()` 仅 `next_turn.pop_front()` 取 **1 条**；`kernel.rs:187-194` 轮开时 claim 一条，claimed 非空则跳过 next-step 兜底；`inbox.rs:101-107` followup 恒 next-turn，状态翻转在 `kernel.rs:181` start_run **晚于投递窗口** → 同批消息全部落 queue。
- **机制修正**（相对 gaps 文档 193 行的旧表述）：dsh 每次 claim 同样只取 1 条 next-turn（deleteCount=1），「合并」来自实时状态分类 + claim 全量 next-step，而非批量领取 next-turn。
- **影响**：subagent 的 relay + notice 拆成两次模型请求、两个 assistant 回复（多倍延迟）；上下文分 turn 呈现 vs dsh 单 turn 单批合并呈现。
- **修复方向**：投递期同步翻转（对齐 dsh wakeDriver 时机）或 claim_open 批量领取。

**16. LLM 错误分类 + 退避重试**（dsh `llm/llm/src/adapter-failure.ts:81-88` 分类学 EMPTY_RESPONSE/RATE_LIMIT/SERVER/TIMEOUT/TRANSPORT + `retry-policy.ts:15-55` Retry-After 透传、500ms→10s 指数退避 + 0.1 jitter、`always` 模式；onion 有 RetryDecision 框架 `crates/onion/kernel.rs:415-447`，但 compaction listener 靠字符串匹配 "context length exceeded"/HTTP 500，`compaction/listener.rs:371-404`，无分类学/退避表/Retry-After）。

**17. compaction per-model 策略**（dsh `compaction/compaction-basic/src/config.ts` modelPolicies 按 provider/model 精确覆写、retainTokens 绝对形态、maxOverflowRetries 独立旋钮；onion `compaction/config.rs:13-27` 单一全局 Config 仅 retainRatio）。

## P2：边界/打磨差异

| # | 差距点 | dsh 证据 | onion 现状 |
|---|---|---|---|
| 18 | PTC：模型以 TS 程序在单次调用中编排工具（timeoutMs + sandbox_permissions 升级） | `core/tool-executor-ptc/src/ptc.ts:353-363,114-116` | 无对应 |
| 19 | web_search provider 面：provider 抽象 + exa/perplexity/deepseek 多实现 | `web/tool-web/src/search.ts:5` | 单一 Anthropic 兼容端点，未配置即失败（`web/search.rs:9-11,105`） |
| 20 | web_fetch 渲染：turndown+GFM（表格/删除线→markdown、剔除 script/hidden） | `web/tool-web/src/fetch.ts:25-30,51-74` | 轻量标签剥离纯文本（`web/fetch.rs:6-7,21-22,203` 自注教学子集） |
| 21 | persistent bash：cwd/env 跨调用持久、scrollback 截断标记 | `shell/tool-bash-persistent/src/index.ts:1-3,20-30` | bash 恒 oneshot（`tools/shell.rs:111-117`），绕行 terminal_*（`terminal/tools.rs:30-36`） |
| 22 | guard 横切：repeat-tool-reminder 连续重复阈值 [3,5,8] 一次性提醒+去重；per-tool timeout-policy 硬失败 | `guard/repeat-tool-reminder/src/index.ts:7-14`、`guard/timeout-policy/src/index.ts:7-9` | RetryNudger 每次工具错误必 steer 无阈值去重（`listeners.rs:244-266`，存在提示洪泛）；无统一 per-tool 超时层 |
| 23 | schedule 面差：create/list/view/edit | `schedule/tool-schedule/src/index.ts:372,394,419,435` | create/list/delete/update（缺 view，`schedule/tools.rs:86,227,261,303`） |
| 24 | hooks-codex 方言 | dsh 有 codex 桥 | 仅 hooks_claude.rs CC 桥 |
| 25 | hooks 触发点覆盖 | 30 个触发点 | 12/30；CwdChanged/FileChanged 只解析不派发（`hooks/config.rs:22-24`） |
| 26 | settings 通用 schema/forms/redact 面 | `packages/settings` | `settings.rs:1-10` 仅对齐 agentDefaultModel.saveSelection |
| 27 | image offload 持久化：durable `image/offload` 事件（seq 账本、跨重放） | `compaction-image-offload/src/image-offload.ts` | 视图级幂等抑制，重放不落事件（`compaction/region.rs`） |
| 28 | goal checkpoint race 栅栏：round attempt 身份预留，防旧 checkpoint 注入新 round | `goal/goal-round-driver/src/index.ts:127-242` | `goal/driver.rs` 自述对齐但无 race fence |
| 29 | max-tokens sticky：任一步触顶后 turn 终态 sticky，后续 completed 不覆盖 | `agent-loop/src/agent.ts:374-379` | 无对应 |
| 30 | jobs 输出反馈粒度：每次读取上报 spill 路径 + gapBefore 缺口标记 | `jobs-local/src/pump.ts:32-33,70-77` | 仅 `truncated` 布尔（`onion-jobs/src/output.rs:44-46`，2000 行环） |
| 31 | 遥测/匿名身份：otel + `.anonymous-user-id` | `telemetry/otel`、`anonymous-user-id/src/index.ts:4-9` | 完全没有；仅 env 实例标签（`paths.rs:69-75`） |
| 32 | 查询授权口径：live/persisted 双源 + shadowed 表面 | `session-query` | 限 cwd 精确相等 + 排除本会话（`session_query/tools.rs:12-13`） |

另：ScopedLayers 组装瀑布与运行时上下文 admission-refresh 为已声明的教学简化（`system_prompt.rs:5-6,13-14`）。

## P3：建议搁置（dsh 专有/深绑定）

- **33** tmux-context / harness-dev-context / time request-zone（dsh 专有或演示级）。
- **34** prompt caching：cacheWrite 计量 + Anthropic signed-thinking 前缀 digest 绑定（`llm-pi-ai/src/prefix-binding.ts:2-11`）；onion 仅读 cached_tokens，cache_write 恒 0（`llm.rs:494-505`）。
- **35** DeepSeek files API + 多 provider 目录（pi-ai）+ token-meter 计价；onion 单一 OpenAI 兼容 SSE（`llm.rs:325`）。
- **36** context 注入管道族（time-context/@file/session-reference）；onion 仅 EnvInjector 单点（`listeners.rs:216-242`）。
- **37** dsh 深绑定：deepseek-account(-platform)、authorization、host 的 directory-picker/frontend-static/plugin-inventory/webserver 路由插件体系、jobs 泵多源乱序窗口语义。
- **38** onion `listeners.rs` 5 个演示 listener 为 ava2 独有，不构成差距，建议标注。

## ava2 独有（不构成差距）

- **App contributes 三类贡献**（skills/mcp/hooks 打包，`app_contrib.rs` + REST `/v1/skills`、`/v1/mcp/servers`）。
- **a2a 三件**（a2a_call/a2a_send/a2a_approve，跨实例 A2A mesh，`assemble.rs:701-702`；入站 `POST /a2a` + `/a2a/sse` + `/.well-known/agent-card.json`，`onion-server/src/router.rs:69-80`）。
- **memory 工具**（memory_search + save_memory，`memory/tool.rs:158`）。
- **MCP 资源三件**（list/read resources + templates，`mcp_resources/seam.rs:221,260,299`）。
- **runtime_clock** 时钟工具（`assemble.rs:566`）。
- **ModelTargetRef 运行时模型热切换**（`model_target.rs`）。
- **listener 瀑布洋葱包裹 + PreStepDecision::Reject 短路**（`listeners.rs:287-309`，dsh 为扁平 extension points）。
- **审批命令内容风险分类**（auto 预设模型复审）+ coding/work 工具面预设体系（`preset.rs:46-49,190`）。
- **onion-sandbox 独立 exe 架构**（可插拔、协议化 JSON 结果；dsh 为进程内 provider，Windows 三档 SandboxMode 语义已对齐）。
- `/v1/tasks` 全局任务管理面、AppHost 应用仓库（安装/卸载/静态托管）、`create_worktree` git worktree 会话、SSE+WS 双事件通道、会话删除连带清理、tool_search/deferred 机制（代码保留）。

## 修复优先级建议

1. **#1 沙箱 fail-closed 化**——唯一 P0；最低成本的一步是「未配置沙箱 → 显式审批兜底」，跨平台 OS 强制另立项。
2. **#15 claim 语义**——唯一有明确运行时观测证据的内核差异（多倍延迟 + 上下文分条）；投递期同步翻转是较小改动。
3. **#16 LLM 错误分类 + #17 per-model compaction**——长会话稳定性直接相关。
4. **#6 MCP 运行期管理面 + #11 崩溃恢复**——长时运行边车的健壮性。
5. **#12 凭据子系统**——api_key 明文落盘为安全问题，建议提前。
6. 其余 P1（工具面补齐、fork、查询、spill、ACP/SDK/webhook）按产品需要排期；P2/P3 视需要逐项打磨或明确搁置。
