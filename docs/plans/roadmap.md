---
title: ava2 后续工作计划（Roadmap）
description: ava2 平台在宿主壳、文档四件套与视觉冒烟修复完成后的分梯队工作计划：边车质量债、桌面壳集成、App 生态深化。
keywords: roadmap, 计划, 边车, 视觉预算, 桌面壳, AppHost
author: AI 助手
date: 2026-11-08
updated: 2026-11-09
status: review
category: plans
---

# ava2 后续工作计划（Roadmap）

> 基线：本计划制定于 2026-11-08。已完成基线——宿主壳布局对齐 dsh + 画布三件套闭环（8ce4c38）、文档四件套（app-dev-guide 7c96e74 / host-kernel-design 03acc1f / sidecar-lifecycle-design f756f0f）、视觉冒烟修复 `wire_message` 图片丢弃（09f672e）。

## 梯队一：边车质量债（视觉能力真实可用的前提）

### 1.1 图片进上下文预算管线（根治 E1.5 估算口径）✅ 已完成（2026-11-08）

**问题**：compaction E1.5 按 data URL 字符数/4 估算图片 token（1920×1920 JPEG ≈ 78K est），超过 threshold 60,416 → 正常大小的截图在返回瞬间被卸载为占位文本。本轮视觉冒烟靠缩图至 1024² 缓解，真实使用（用户截图几百 KB）仍必被卸载。

**方案**（对齐 ava `ava-harness/src/tools/image_budget.rs` + `meter/pricing.rs` 已验证的两层方案）：

1. **进上下文前预算**（`read_image` 工具返回前）：解码 → 超过 `MAX_IMAGE_PIXELS=1.15M` 像素则长边缩至 1568 重编码；超过 `MAX_IMAGE_BYTES=1MB` 则压缩重编码；限制上下文历史图片 ≤4 张（更早的换占位）。
2. **估算封顶**（`compaction/estimate.rs`）：预算后的单图估算固定 `IMAGE_TOKEN_ESTIMATE=2048`（≈1.15MP 按视觉 tile 折算的保守值），不再按 data URL 字符数折算。

**落地**：新增 `onion/src/media.rs`（`budget_blocks_async` 像素/字节预算 + `evict_old_images` 数量封顶，接入 `read_image` 与 kernel 请求装配）；`estimate.rs` 图片改固定单图估算；单测 78 例通过。冒烟：1920×1920 PNG 经 `read_image` 被精确描述且无 E1.5 卸载（in=14562 含图 ≈2-4K）。详见 `docs/design/chatapp-rendering-design.md` §8.2。

**验收**：1920×1920 截图经 `read_image` 进入模型且可被准确描述（不触发 E1.5 卸载）；`compaction` 单测覆盖估算口径。

**依赖**：`image` crate 解码/缩放（评估 cargo-deny 合规与构建开销）。

### 1.2 compaction no-shrink 报错修复 ✅ 已完成（2026-11-08）

**问题**：E1.5 卸载后摘要管线报 `framed summary (513 tok) does not shrink the shadowed region (80 tok)` 后降级继续。shadowed region 仅 80 tok 时根本不应发起摘要事务——发起条件缺下限保护。

**方案**：摘要事务前置校验 shadowed region 估算 ≥ 阈值（如 ≥ 2K tok）才执行；低于阈值直接跳过。

**落地**：`region.rs` 新增 `MIN_SHADOWED_TOKENS = 2_048`；`listener.rs` 摘要事务在范围选择后、bracket 打开前校验 shadowed 估算，低于下限直接 `Nothing` 跳过（无 CompactionStart/End 事件）；回归用例 `tiny_shadowed_region_skips_transaction`。

**顺带**：exit -1 偶发崩溃（当日一次，未复现）保持观察；若复现抓 panic 栈归入本项处理。

## 梯队二：从 web 走向客户端

### 2.1 桌面壳集成（onion-server 生命周期托管）✅ 已完成（2026-11-08）

**背景**：Tauri 壳原型在 ava2 初始提交（3dbaba0）已含完整生命周期托管（`src-tauri/src/sidecar.rs`：spawn/环境注入/日志滚动/进程树终止 + `lib.rs` 的 `backend_base` 命令 + `wait_ready` TCP 探活 + 标题栏状态点三态），本轮完成端到端验收与文档收口。

**方案**（`docs/design/sidecar-lifecycle-design.md` 的「桌面端集成契约」章节落地，参照 ava `ava-app/src-tauri/src/sidecar.rs`）：

1. Tauri `setup` 阶段 spawn `onion-server.exe`，注入 `ONION_HOME` / `ONION_SERVER_PORT` / `ONION_SERVER_CORS_ORIGINS`（tauri://localhost）；
2. 窗口销毁时 kill 子进程，防残留（排查端口占用日志）；
3. 日志落 `<data_home>/logs/onion-server.log`（滚动 `.old`）；
4. 前端基址解析走既有 `env.resolveBackendBase`（invoke backend_base → `?backend=` → 默认 9800）；
5. 边车健康探活：壳启动后轮询 `/v1/health`，就绪前前端显示连接态。

**验收**：`tauri dev` 一键启动桌面窗口，边车随窗口生命周期启停，chatapp 在 WebView2 内完成一轮真实对话。

**落地**：`dev.ps1` 一键启动验收通过——壳 spawn 边车（`ONION_SERVER_PORT`/`ONION_SERVER_CORS_ORIGINS` 注入，日志落 `<ONION_HOME>/logs/onion-server.log` 滚动 `.old`）；WebView2（msedgewebview2）与 9800 建立 TCP 连接；REST 驱动一轮真实对话（Qwen3.8-27B 回复「收到」，turn completed，in=9883/out=17）；WM_CLOSE 关窗后边车进程退出、端口释放（kill 子进程树生效）。CORS 四元来源（vite 1420 双写 + `http://tauri.localhost` + `tauri://localhost`）由 onion-server `cors_layer` 逐一解析放行。

### 2.2 App 设置页设计 + 多页面 plugin 装配深化 ✅ 已完成（2026-11-08）

`docs/index.md` 阅读顺序遗留项：App 运行态设置（enable/disable 已有 REST 面，缺 UI）；多 App/多页面的 plugin 装配规则深化（`contributes.apps[]` 页面注册 → 认领表 → lazy-cache 键协同）。

**落地**：App 管理卡片内联设置表单（按页级 schema 渲染 config，保存→校验→`registry.reload` 使新值经 `ctx.settings` 注出）；`registry.load` 逐页 schema 校验（`page.config ?? manifest.config`，任一页非法整 App failed）+ 页级 `hostApi` 生效（能力面 granted 按页收窄）；卸载/保存重载前清 lazy-cache（同 id 覆盖重装不再跑旧模块）。规则固化见 `docs/solutions/app-settings-and-multipage-assembly.md`。验收：CDP 驱动 headless Edge 七步真实交互冒烟全通过（双页认领不同主视图、越界校验拦截、保存→重载→新值注出、页乙页级覆盖过滤、chatapp/hello 回归）。

## 梯队三：场景深化

### 3.1 第二个真实 App（自验证）✅ 已完成（2026-11-09）

用 canvas 三件套 + agent 通道再做一个真实 App，吃自己的狗粮验证 `app-dev-guide.md` 的契约完备性，反哺文档与 host API 版本。

**落地**：新增 `apps/sessionboard/`（会话工作台，裸 ESM）——五类 slot（surface.main 会话仪表盘 / surface.canvas 对话试验台 / nav.sidebar / panel.sidebar 会话速览 / panel.canvas 运行状态）+ 五类 inject 面（slots/agent/events/storage/settings）全量消费；试验台经 agent 通道真实对话往返（Qwen3.8-27B），storage 记忆选中会话，config schema（pageSize/confirmDelete）走声明式设置。CDP 冒烟八步全通过（安装 → 装配 → 新建会话 → storage 预选 → panel 空闲态 → 真实往返 → 回归）。契约观察反哺 guide：`listSessions` 排序未定义、`snapshot` 帧即就绪信号、画布 tab 切换卸载语义、面板跟随模式（storage + 自定义总线事件）。详见 `docs/solutions/sessionboard-app.md`。

### 3.2 交互验证清单（用户手动核对）

浏览器桥接不可用期间遗留：侧栏折叠/拖拽（264–420px/56px）、画布开合、canvas:home 启动器打开「会话总览」、panel.canvas「运行状态」面板渲染。

### 3.3 chatapp 会话三控件（工作区 / 模型 / 审批模式切换）✅ 已完成（2026-11-09）

**背景**：chatapp header 仅有会话切换，缺工作区、模型、审批模式的切换能力；onion-server 侧亦无对应 REST 端点（模型路由与审批配置此前只在会话装配期定死）。

**落地**：
- 后端：SessionHandle 持有共享 `ModelTargetRef`（kernel 与 llm 同源，`swap_current` 双侧生效）与共享 `ApprovalConfig`（ApprovalGate 同源，运行期可写）；新增 `PUT /v1/sessions/{id}/model`（ProviderCatalog 校验 + `SessionEvent::ModelSelection` durable 审计）与 `PUT /v1/sessions/{id}/approval`（三值白名单 + `build_approval_config` 重建）；SessionInfo 增 `model` / `approvalPreset` 读面。
- 契约：AgentChannel 增 `setSessionModel` / `setApprovalPreset`（均返回更新后 SessionInfo）。
- 前端：chatapp header 四组控件（会话 / 工作区 / 模型 / 审批）：工作区下拉含近用列表（storage 持久化，前 5 个）+ 自定义路径输入；模型下拉按 provider 分组（`listModels`）；审批下拉三值；`lastModel` 跨会话延续。
- 边界：权限根装配期固定——工作区切换 = `createSession(cwd)` 新会话；模型切换审计落 durable 事件但冷恢复不重建 target。

**验收**：`cargo test -p onion-server` 34 例全绿；CDP 冒烟九步全通过（安装 → 装配 → 新会话 → 审批切 auto → 模型切 DeepSeek-V4.1-Flash-1017 → 真实对话往返 → 工作区切 hello → lastModel 延续 → 刷新持久化）。详见 `docs/solutions/chatapp-controls.md`。

## 执行顺序建议

1.1 → 1.2（同一管线，先预算后摘要保护）→ 2.1（桌面壳）→ 2.2 → 3.1 → 3.3 ✅ 全部完成（2026-11-09）。每项独立提交；涉及行为变化的跑真实 API 冒烟（local 供应商）。
