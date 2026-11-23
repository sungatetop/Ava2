---
title: 工具审批「工作区写模式仍需逐次审批」修复方案
description: 梳理 ava2 onion 工具审批在 workspace-write 预设下对基础 shell 命令逐次弹卡的完整判定链路与六个根因，并给出分阶段修复方案、契约变更、测试计划与待决问题。
keywords: 工具审批, approval gate, workspace-write, 风险分类, shell, allowed-once, onion, ava2
author: AI 助手
date: 2026-11-11
updated: 2026-11-11
status: done
category: solutions
---

# 工具审批「工作区写模式仍需逐次审批」修复方案

## 1. 问题现象

会话审批预设为「工作区写」（`workspace-write`）时，最基础的命令行调用仍逐次弹审批卡。典型现场：

```json
{
  "tool": "bash",
  "input": { "command": "sleep 0.5", "description": "sleep 0.5（第 2/20 次）" }
}
```

模型侧是一个 20 次的轮询循环，用户侧就是 20 张内容几乎相同的审批卡。用户的预期是「工作区内的写操作不该问」（前端文案与系统提示词都这么承诺），实际体感是「除了 grep/cat 之外什么都要问」。

## 2. 现状判定链路

以 `bash` + `sleep 0.5` 为例，一次调用的完整准入路径：

```
BashTool::classify_risk(&call)
  └─ classify_shell_command("sleep 0.5")
       └─ "sleep" 不在 READONLY_COMMANDS → ToolRisk::Escalating
ApprovalGate::tool(ctx, call, next)
  └─ decide_full("bash", Escalating, preset)
       └─ deny? no / allow? no
       └─ decide(Escalating, WorkspaceWrite, Ask)
            └─ SandboxLevel::allows(WorkspaceWrite) 只放 ReadOnly|WorkspaceWrite → false
            └─ ApprovalPolicy::Ask → Decision::Escalate
  └─ reviewer = None（workspace-write 预设不挂 reviewer）
       └─ 新建 ApprovalRequest → 挂起人工复决 → 前端弹卡（allowed-once）
```

关键代码位置：

| 环节 | 位置 |
| --- | --- |
| shell 风险分类器 | `crates/onion/tools/shell.rs` `classify_shell_command`（L293-L308） |
| 只读命令白名单 | `crates/onion/tools/shell.rs` `READONLY_COMMANDS`（L314-L331） |
| 沙箱允许集 | `crates/onion/approval.rs` `SandboxLevel::allows`（L43-L52） |
| 判定内核 | `crates/onion/approval.rs` `decide` / `decide_full`（L82-L91、L268-L278） |
| 审批 gate | `crates/onion/listeners.rs` `impl Listener for ApprovalGate`（L311-L409） |
| 审批请求上抛 | `crates/onion-server/src/session.rs`（L1699-L1731） |
| 前端审批卡 | `apps/chatapp/src/items.tsx` `ApprovalCardView`（L492-L513） |
| 预设文案 | `apps/chatapp/src/composer-chips.tsx`（L13-L17） |

## 3. 根因

### R1 风险分类是「正向白名单」，漏一个问一次

`classify_shell_command` 只把显式列举的只读命令判为 `ReadOnly`，其余一律 `Escalating`，且 `unknown_command_escalates_fail_closed` 把这个行为写成了测试契约（`tools/shell_tests.rs` L97-L100）。`sleep`、`true`、`test`、`printf`、`node`、`python`、`npm run`、`cargo test`、`mkdir`、`touch` 都不在表内，因此「基础命令行工具也要审批」是当前设计的必然结果。

### R2 shell 的风险模型只有两档，永远产不出 `WorkspaceWrite`

`ToolRisk` 有三档，但 shell 分类器只返回 `ReadOnly` / `Escalating`（`shell.rs` L228-L231、L258-L261）。于是 `workspace-write` 沙箱对 shell 的实际语义退化为「纯只读直通、其余全问」，与以下承诺矛盾：

- 前端：`workspace-write` = 「工作区之外的写操作需逐次审批」（`composer-chips.tsx` L14）；
- 系统提示词：「默认 workspace-write 预设下工作区内写入直通，不逐次审批」（`crates/onion/assemble.rs` L155）。

根子在于：判定维度是**命令名**，而承诺的语义是**写入位置**。

### R3 桌面部署未启用 OS 级沙箱

`SandboxConfig.mode()` 取自 `preset.sandbox`，但 `sandbox_executable` 依赖与 sidecar 同目录的 `onion-sandbox.exe`（`crates/onion/config.rs` L184-L194）。当前 `src-tauri/resources/` 只有 `onion-server.exe`，`dev.ps1` 也只构建/拷贝 server（L34-L53）。

后果是双向的：

1. shell 命令直接由 `std::process::Command` 执行，没有位置级遏制兜底；
2. `sandbox_denied` 升级通道（`shell.rs` L186-L211）永远不触发，表面上看不出问题，一旦补上沙箱就会暴露 R5。

### R4 审批只有 `allowed-once`，没有会话级记忆

`ApprovalDecision` 只有 `AllowedOnce` / `Rejected` 两个变体（`approval.rs` L63-L69），全仓不存在 `always` / allowlist 概念。`ApprovalGate` 每次 `Escalate` 都无条件新建请求（`listeners.rs` L366-L409），服务端逐条 push pending（`onion-server/src/session.rs` L1700-L1731），前端逐条渲染卡片。循环 N 次 = N 张卡。

### R5 存在「双次审批」的潜在路径

人工批准后，命令仍在当前 preset 的 OS 沙箱下执行；若被沙箱判为 `sandbox_denied`，`run_shell` 会通过 `SandboxEscalator::escalate` **再发一次同工具审批**（`shell.rs` L190-L211），第二次的卡片文案与第一次完全相同，用户无法区分。当前因 R3 休眠。

### R6 `SandboxEscalator::exec_danger` 是 fail-open 的

`exec_danger`（`shell.rs` L62-L77）把**共享** `ApprovalConfig.preset.sandbox` 临时改成 `DangerFullAccess`，执行完再改回：

- 并发：同会话其它工具调用在窗口期内读到 `danger-full-access`，OS 沙箱随之放开；
- 取消：future 在 `run.await` 处被 drop（会话退出 / 中断）时，恢复代码不执行，沙箱级别**永久停在 `danger-full-access`**。

### 次要问题

- `auto` 预设的 `LlmReviewer` 对每次升级调用都发一次 LLM 复审（30s 超时、fail-closed，`approval.rs` L219-L266）。循环场景下延迟与误拒都被放大，且拒绝理由只有固定串 `rejected by reviewer`，模型无法据此改正。
- `terminal_open` / `terminal_send` / `terminal_signal` / `terminal_close` 硬编码 `Escalating`（`crates/onion/terminal/tools.rs` L241-L243），与 shell 一样逐次弹卡。
- `ApprovalConfig::PRESET_NAMES` 含 `custom`、`for_preset` 仍保留 `custom` 分支，但前端与 `onion-server/src/rest/control.rs`（L129）只暴露三档；`read-only` 预设已在 ava2 移除。契约面存在冗余，建议后续清理（本方案不涉及）。

## 4. 目标语义契约

修复后，`workspace-write` 预设对三类操作给出三档结果：

| 操作类别 | 示例 | 风险档 | 结果 |
| --- | --- | --- | --- |
| 无副作用 / 纯读 | `sleep`、`cat`、`git status`、`grep` | `ReadOnly` | 直通（审计 `via=sandbox`） |
| 写工作区内、无网络、非破坏 | `mkdir build`、`touch a.txt`、`npm run build`、`cargo test` | `WorkspaceWrite` | 直通（**前提**：OS 沙箱就位，越界由沙箱拒绝） |
| 破坏性 / 越界 / 不可静态判定 | `rm -rf`、`sudo`、`curl \| sh`、`$(...)`、`-exec` | `Escalating` | 人工复决（可「本会话允许」） |

不变量：

1. fail-closed 不变——分类不确定一律 `Escalating`，`Rejected` 仍是唯一非放行结果之一；
2. 审计不降级——任何放行都要落 `SessionEvent::ApprovalAudited`，`via` 新增 `operator-session`；
3. 判定内核（`decide` / `decide_full`）的优先级不变：`deny > allow > 沙箱允许集 > 审批策略`。

## 5. 分阶段方案

### 阶段 1：不依赖 OS 沙箱的体验修复（P0）

只做「不可能产生越界副作用」的部分，风险可控，可独立合入。

**1.1 只读白名单扩容**（`tools/shell.rs`）

补入无副作用命令：`sleep`、`true`、`false`、`test`、`printf`、`basename`、`dirname`、`realpath`、`seq`、`which`（已含）、`nproc`、`get-itemproperty` 等只读/纯计算类。判定逻辑不变，只扩表。

同时补测试：新增条目必须出现在 `read_commands_are_readonly` 用例中；`DANGEROUS_PATTERNS` 对 `sleep` 之类的无参数命令不引入误伤。

**1.2 会话级「允许该命令」**（`approval.rs` + `listeners.rs` + 前端）

```rust
pub enum ApprovalDecision {
    AllowedOnce,
    /// 本会话内对该工具（`Escalating` 场景下按命令首词粒度）持续放行。
    AllowedForSession,
    Rejected,
}
```

- `ApprovalGate` 持有会话级 allow 表：`Mutex<HashMap<String, SessionGrant>>`，`SessionGrant { tool: String, command_prefix: Option<String> }`；
- 命中 allow 表的调用直接 `ToolAdmission::Allow`，审计 `via="operator-session"`；
- 前端审批卡从两个按钮扩为三个：`拒绝` / `批准` / `本会话允许`（第三项仅在 `Escalating` 且带 `command` 参数时出现，展示为「本会话允许 `bash: sleep`」）；
- REST 契约：`POST /v1/sessions/{id}/approvals/{requestId}` 的 `decision` 增加 `approve_for_session`；`onion-server` 侧把三种回执透传到 `ApprovalDecision`。

**1.3 在飞审批去重**（`listeners.rs`）

以 `(tool, 归一化命令首词)` 为键，若已有同键请求在飞，则复用同一个 pending（多个等待者共享一次裁决），避免模型并发/循环时弹卡风暴。

**1.4 `exec_danger` 改为一次性 override**（`shell.rs`）

不再改写全局 `preset.sandbox`，改为把 `SandboxLevel::DangerFullAccess` 作为一次性参数传给 `OneshotExecutor::exec`（或包裹一个 `Drop` 恢复的 guard），消除 fail-open 与并发读写窗口。

**1.5 修 R5 的双次审批文案**

`SandboxEscalator::escalate` 的审批卡增加来源标记（`source=gate|sandbox-retry`），并在 `ApprovalRequest` 上带一个 `reason` 字段，前端据此展示「沙箱拒绝后申请越界执行」，避免与第一次审批混淆。

### 阶段 2：让 workspace-write 语义真正成立（P1-P2）

**2.1 部署 OS 沙箱**

`dev.ps1` 增加 `cargo build -p onion-sandbox` 并把 `onion-sandbox.exe` 与 `onion-server.exe` 一并拷到 `src-tauri/resources/`。这是阶段 2 其余改动的**前置条件**——没有位置级遏制，放开 `WorkspaceWrite` 就是真放行。

**2.2 shell 产出 `WorkspaceWrite` 档**

在 `classify_shell_command` 中新增第三类判据 `WORKSPACE_WRITE_COMMANDS`（写工作区、无网络、非破坏：`mkdir`、`touch`、`cp`、`mv`（同目录）、`npm run`、`cargo build/test`、`node`/`python` 执行脚本文件、`git add/commit`）。判定顺序：

```
命令替换 / 反引号 / 脚本块 / 危险参数 → Escalating
含网络命令（curl / wget / npm install / pip install / git push）→ Escalating
仅破坏性目标（rm -rf、del /s、rmdir /s）→ Escalating
全部段命中只读表 → ReadOnly
全部段命中读表 ∪ 工作区写表（且无越界重定向）→ WorkspaceWrite
其余 → Escalating
```

**2.3 文案与文档对齐**

`composer-chips.tsx` 三档 hint、`assemble.rs` L155 与 `onion-server/src/session.rs` L837 的系统提示词措辞，统一改成「只读 + 工作区内写直通；越界写、破坏性命令、网络与不可静态判定命令逐次审批」。

## 6. 契约变更清单

| 面 | 变更 | 兼容性 |
| --- | --- | --- |
| `onion::approval::ApprovalDecision` | 新增 `AllowedForSession` | 非穷尽匹配处需补分支；`matches!(.., AllowedOnce)` 判断会自然变为拒绝，**必须全量排查调用点** |
| `onion::listeners::ApprovalRequest` | 新增 `reason: Option<String>`（来源标记） | 内部结构体，无外部序列化 |
| REST `POST /v1/sessions/{id}/approvals/{requestId}` | `decision` 增加 `approve_for_session` | 向后兼容（旧值仍有效） |
| `apps/chatapp` 审批卡 | 增加第三按钮 + 来源标签 | 纯前端 |
| `ToolRisk` | 不变 | — |

`AllowedForSession` 引入时，最关键的是**不要**让 `matches!(receiver.await, Ok(ApprovalDecision::AllowedOnce))` 这类现有判断静默降级为拒绝。已知需要同步的位置：`listeners.rs` L380-L389（gate 主路径）、`shell.rs` L58（`SandboxEscalator::escalate`）、`onion-server/src/session.rs` L1710（非交互自动放行）、TUI 的 `/approve` 命令处理。

## 7. 测试计划

**单元测试（`cargo test -p onion`）**

- `read_commands_are_readonly`：新增的 `sleep` / `true` / `printf` 等条目全部命中 `ReadOnly`；
- `workspace_write_commands_are_workspace_write`（阶段 2）：`mkdir` / `cargo test` / `npm run build` 命中 `WorkspaceWrite`；`rm -rf` / `curl` / `sudo` / `$(...)` 仍 `Escalating`；
- `approval_tests.rs`：`AllowedForSession` 语义——首次批准后同键调用不再产生新请求，审计 `via=operator-session`；
- `exec_danger`：取消 future 后 `preset.sandbox` 必须保持原值（回归 R6）。

**集成测试（`cargo test -p onion-server`）**

- 审批回执三值透传：`approve` / `approve_for_session` / `deny` 分别映射到 `AllowedOnce` / `AllowedForSession` / `Rejected`；
- 非交互会话（`interactive = false`）自动放行路径不因新变体改变行为。

**前端**

- `npx tsc --noEmit` + `npm run build` 通过；
- 审批卡三按钮渲染条件（带 `command` 参数才展示「本会话允许」）。

**冒烟（按 ava2 规则必须截图留证）**

1. `.\dev.ps1` 启动，会话预设保持「工作区写」；
2. 让模型执行 `sleep 0.5` 循环 20 次：预期**只弹 1 张卡**，批准后其余 19 次直通；
3. 让模型执行 `mkdir smoke-dir`：阶段 1 仍弹一次卡（预期），阶段 2 在沙箱就位后应直通；
4. 让模型执行 `rm -rf`：必须保持逐次审批，且「本会话允许」不应被用作绕过手段（破坏性命令默认不提供该按钮）。

## 8. 落地顺序与风险

| 阶段 | 内容 | 风险 | 回滚 |
| --- | --- | --- | --- |
| 1.1 | 白名单扩容 | 低：仅新增无副作用命令；需防 `sleep` 类命令被拼接滥用（`&&` 分段校验已覆盖） | 回退表项即可 |
| 1.2 | 会话级允许 | 中：放行粒度若按工具名而非命令族，等于绕过审批，必须按命令首词收敛 | 默认不展示第三按钮即可降级为原行为 |
| 1.3 | 在飞去重 | 低：仅影响 pending 数量，不改变裁决语义 | 移除合并逻辑 |
| 1.4 | 一次性 override | 低：消除 fail-open，属纯收紧 | — |
| 1.5 | 来源标记 | 低：纯展示 | — |
| 2.1 | 部署沙箱 | 中：Windows 受限 token + ACL 在新环境可能影响合法写入，需先跑冒烟矩阵 | 不拷 `onion-sandbox.exe` 即回到直通 |
| 2.2 | `WorkspaceWrite` 档 | **高：依赖 2.1 完成**，未部署沙箱时合入等于对任意写命令自动放行 | 分类器回退为两档 |

阶段 1 可独立评审合入（预计 diff < 300 行）；阶段 2 必须与沙箱部署同批验证，不建议拆开单独合入。

## 9. 待决问题

1. 「本会话允许」的粒度：按工具名（`bash` 全放）、按命令首词（`sleep`）、还是按完整命令串？建议**命令首词**——既能覆盖循环重复，又不至于一条 `rm` 授权放行所有删除。
2. 会话级授权是否落盘（跨会话保留）？建议**不落盘**，与 `onion-server/src/session.rs` L1404 的会话级 `approval_config` 生命周期一致。
3. 破坏性命令（`rm -rf` / `del /s` / `format`）是否彻底不提供「本会话允许」按钮？建议**不提供**。
4. `read-only` 预设是否恢复？当前三档（工作区写 / 自动 / 全放行）与 dsh 默认表已不一致，建议单开一篇 ADR 处理，不并入本方案。
5. `auto` 预设的 LLM reviewer 是否要做结果缓存（相同 `(tool, 命令)` 只在首次复审）？与 1.3 去重可合并设计，但涉及缓存失效策略，暂列为下一步。

## 10. 落地记录（阶段 1、2 均已完成，2026-11-11）

### 10.1 代码改动

| 项 | 位置 | 内容 |
| --- | --- | --- |
| 1.1 | `crates/onion/tools/shell.rs` | `READONLY_COMMANDS` 扩容：`sleep` / `true` / `false` / `test` / `printf` / `expr` / `seq` / `nproc` / `uptime` / `free` / `basename` / `dirname` / `realpath` / `readlink` / `get-itemproperty` / `split-path` / `join-path` / `compare-object` |
| 1.2 | `crates/onion/approval.rs` | 新增 `ApprovalDecision::AllowedForSession` 与 `is_allowed()`；新增 `session_grant(tool, input)` 归一化键（简单命令按命令首词、组合命令按完整命令、破坏性命令返回 `None`） |
| 1.2 | `crates/onion/listeners.rs` | `ApprovalGate` 增加会话级授权表（`HashSet<String>`，不落盘）+ 命中直通（审计 `via=operator-session`）；`Reviewer` 路径改用 `is_allowed()` |
| 1.2 | `crates/onion-server/src/rest/inputs.rs` | `decision` 支持 `approve_for_session` → `AllowedForSession` |
| 1.2 | `apps/chatapp/src/items.tsx` | 审批卡三按钮：拒绝 / 批准 / 「本会话允许 {tool}: {cmd}」；破坏性命令与组合命令不展示第三项 |
| 1.2 | `src/host/{types,agent}.ts`、`apps/chatapp/src/{chat-view,chat-types}.tsx` | `replyApproval` 与 `PendingCard` 契约加入 `approve_for_session` / `reason` |
| 1.3 | `crates/onion/listeners.rs` | 在飞审批去重：同键（可授权键或完整输入）共享一次人工裁决，`Inflight { decision, notify }` |
| 1.4 | `crates/onion/tools/shell.rs`、`shell_exec/{oneshot,sandbox}.rs`、`assemble.rs` | 删除 `SandboxEscalator::exec_danger`（不再改写共享 `preset.sandbox`）；新增 `OneshotExecutor::exec_with_mode` + `SandboxRunner::exec(mode)` 一次性覆盖 `DangerFullAccess` |
| 1.5 | `crates/onion/tools/shell.rs`、`onion-server/src/{session,rest/*}.rs`、前端 | `ApprovalRequest.reason` / `PendingKind::Approval.reason` / `SessionNotice::ApprovalRequested.reason` 打通；沙箱重试标记 `sandbox-retry`，审批卡顶条显示「沙箱拒绝后申请越界执行」 |

破坏性命令（`rm` / `del` / `format` / `sudo` / `kill` 等）即使用户回 `approve_for_session` 也只按一次性放行（`listeners.rs` 的 `AllowedForSession` 分支要求 `session_grant` 非空）。

### 10.2 验证结果

- `cargo test -p onion --lib -- approval::tests listeners::tests tools::shell::tests`：23 passed（含新增 `session_grant_*`、`allowed_for_session_covers_same_command_family`、`allowed_for_session_never_covers_destructive_commands`、`concurrent_same_key_requests_share_one_decision`、`side_effect_free_commands_are_readonly`）。
- `cargo test -p onion-server`（全量）与 `--test rest_api`（含新增 `approve_for_session` 回执用例）通过。
- `npx tsc --noEmit`（宿主 + chatapp）与 `npm run build`（chatapp）通过。
- CDP 冒烟 `.ava2-smoke/approval-session-smoke.mjs`（真实模型链路，截图见 `.ava2-smoke/shots/`）：
  1. `bash sleep 0.5` → **0 张审批卡**（只读白名单生效）；
  2. `bash mkdir smoke-dir` → 弹卡且按钮为 `拒绝 / 批准 / 本会话允许 bash: mkdir`；
  3. 点「本会话允许」后 `mkdir smoke-dir-two` → **0 张审批卡**（会话级授权生效）；
  4. `rm -rf ./smoke-should-not-exist` → 仍弹卡且仅 `拒绝 / 批准`（无会话级授权）。

### 10.3 落地记录（阶段 2 已完成，2026-11-11）

#### 代码改动

| 项 | 位置 | 内容 |
| --- | --- | --- |
| 2.1 | `dev.ps1` | `cargo build -p onion-server -p onion-sandbox`，`@("onion-server.exe","onion-sandbox.exe")` 一并拷贝到 `src-tauri/resources/`（同目录约束：`ProcessSandboxRunner` 从边车 exe 同目录定位沙箱） |
| 2.2 | `crates/onion/tools/shell.rs` | 新增 `WORKSPACE_WRITE_COMMANDS` 与三档判定（deny > 读白名单 > 读写白名单 > Escalating）；无沙箱 exe 时 `WorkspaceWrite` 自动降级 `Escalating`（不因部署缺件而 fail-open） |
| 2.2 | `crates/onion/shell_exec/` | `confine_to_sandbox`：`WorkspaceWrite` 且未声明后台执行 → 走 `onion-sandbox --mode workspace-write`；`run_in_background` 不受 OS 沙箱覆盖 → 保持 `Escalating` 逐次审批 |
| 2.3 | `composer-chips.tsx` / `assemble.rs` / `session.rs` | workspace-write hint 与系统提示词统一为「只读 + 工作区内写直通；越界写、破坏性命令、网络与不可静态判定命令逐次审批」；平台化 shell 工具（Windows 注册 `PwshTool`，不注册 bash——bash 是 macOS/Linux 命令） |
| 2.4 | `crates/onion-sandbox/src/platform/windows.rs` | ACL 收敛：`ensure_write_acl`（workspace-write，幂等：已有授权即跳过）/ `revoke_write_acl`（read-only，撤销残留授权）/ `granted_roots`（沿祖先目录查授权根）/ `dacl_grants_write`（区分直接 ACE 与继承 ACE）；`CREATE_NO_WINDOW`（无控制台子进程不挂起）；`pwsh_args` 包 `$ProgressPreference` 修 ConstrainedLanguage 下 CLIXML 噪声；沙箱拒绝判定改为「输出含拒绝标记」（去空白匹配，兼容 PowerShell 折行）而非退出码 |
| 2.5 | `apps/chatapp/src/items.tsx` | 沙箱重试卡（`reason=sandbox-retry`）不提供「本会话允许」按钮：沙箱已证实越界的命令不做会话级批量授权，与破坏性命令逐次审批的承诺一致 |

#### 关键决策与实测陷阱

1. **ACL 授权幂等 + 进入只读档撤销**：带继承的 `SetNamedSecurityInfoW` 会同步递归传播整棵子树，含 `target/` 的巨型工作区（实测 26 万+ 条目）单次传播 60s+。若每条命令「授权 + 回收」等于工作区写不可用。改为只在缺失时授权（只有首条命令付传播成本），并保留授权在本工作区；只读档进入时若发现残留授权（同一 logon 会话共享 logon SID）则撤销，避免只读档被悄悄放宽成可写档（fail-open）。
2. **restricting SID 不能去掉 logon SID**：把受限 token 的 restricting 集合改成仅 `{Everyone}` 会导致子进程直接起不来（`0xC0000142` STATUS_DLL_INIT_FAILED）。保持 restricting 恒为 `{logon SID, Everyone}`，可写性由 DACL 控制。
3. **沙箱拒绝以输出标记判定**：正常业务失败（测试用例失败、`test -f` 返回 1）也是非零退出，按退出码判会误报；PowerShell cmdlet 写失败是非终止错误（报错后仍 exit 0），按退出码判又会漏判。以「输出含 access denied / UnauthorizedAccessException / 拒绝访问」为准，且匹配前去掉空白（PowerShell 按控制台宽度把长标识折行）。
4. **后台执行不覆盖**：`run_in_background` 的命令绕过沙箱 runner，无法位置遏制 → 维持 `Escalating` 逐次审批（提示词已声明）。
5. **冒烟工作区预置**：chatapp 侧边栏只显示 `eventCount > 0` 的会话且按 workingDir 分组、非当前组折叠；冒烟先经 REST 创建根为 `.ava2-smoke/ws` 的预置会话并发一条首条消息，再在 UI 中展开分组附着，避免会话根落成巨型工作区（会触发 60s+ 的 ACL 传播）。

#### 已知限制

- 授权不回收：workspace-write 会话结束后，工作区目录 DACL 上保留 logon SID 的写授权 ACE（登出后成为悬空 SID）。安全边界仍在受限 token（进程级），DACL 只是辅助。
- 中断的首次传播可能只完成一部分子树；`ensure_write_acl` 沿祖先查到授权 ACE 即跳过，极端情况下部分深层目录可能未获授权（表现为该处写被拒并弹「沙箱重试」卡，fail-closed 不放大权限）。
- logon SID 只在会话内有效，跨会话残留 ACE 无法用新 logon SID 撤销，需管理员手动 `icacls <dir> /remove:g "*S-1-5-5-0-*"`。

#### 验证结果

- `cargo test -p onion-sandbox`：5 passed（拒绝标记匹配：英文直排 / PowerShell 折行 / 中文 / 业务失败不误报 / 参数引号）。
- `cargo test -p onion-server`：51 passed（单元 45 + rest_api 3 + session_cwd_isolation 3）。
- 沙箱 exe 直测（`--cwd .ava2-smoke/ws`）：workspace-write 内建目录成功（~1.7s，幂等跳过未变慢）；read-only 建目录被拒 `sandbox_denied=true`；workspace-write 写 `C:\Windows\Temp` 被拒 `sandbox_denied=true`。
- CDP 冒烟 `.ava2-smoke/workspace-write-smoke.mjs`（真实模型链路，截图 `.ava2-smoke/shots/`）：
  1. 预置会话（根 = 冒烟工作区）+ 审批预设 workspace-write；
  2. `pwsh mkdir`（工作区内写）→ **0 张审批卡**直通；
  3. `pwsh Set-Content C:\Windows\Temp\...`（越界写）→ 沙箱拒绝 → 「沙箱拒绝后申请越界执行」卡，仅 `拒绝 / 批准`；拒绝后目标文件不存在；
  4. `pwsh Remove-Item`（破坏性）→ 逐次审批，仅 `拒绝 / 批准`（无会话级授权）。

### 10.4 补漏（pwsh 无害 cmdlet 白名单，2026-11-11）

#### 现象与根因

chatapp `/plan` 冒烟中实测：workspace-write 下模型调用 `pwsh Start-Sleep -Milliseconds 500` 被挂起人工复决。根因是只读白名单只收录了 bash 形态的 `sleep`，pwsh 的 `start-sleep`（及同类的纯输出 `write-output` / `write-host`）缺失——分类器 fail-closed 升级为逐次审批，与「等待 / 纯输出无副作用」的分类意图不符（用户反馈：workspace-write 模式下 pwsh 不应该出现不合理的审批请求）。

#### 改动与验证

- `crates/onion/tools/shell.rs` `READONLY_COMMANDS`：新增 `start-sleep`、`write-output`、`write-host`（`echo` 组与纯等待组各归其位）。
- `crates/onion/tools/shell_tests.rs` 新增两测：`benign_pwsh_cmdlets_are_readonly`（含组合命令与管道 `Start-Sleep 1 | Out-Null`）、`non_whitelisted_pwsh_verbs_still_escalate`（`Start-Process` / `Invoke-Expression` 等 fail-closed 不放水）。
- `cargo test -p onion shell::`：12 passed；REST 端到端（默认 workspace-write 预设真实模型轮次调 `pwsh Start-Sleep`）全程 `/pending` 为空、轮次正常收束。
