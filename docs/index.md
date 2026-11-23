---
title: ava2 文档入口
description: ava2（AppHost 原型落地实践）文档总入口，说明六分类目录结构、阅读顺序与维护约定。
keywords: ava2, AppHost, 文档索引, onion-server, Tauri
author: AI 助手
date: 2026-11-07
status: draft
category: references
---

# ava2 文档入口

ava2 = Tauri 桌面壳 + AppHost 可插拔 App 契约 + onion-server Agent 运行时边车。

## 目录结构

| 目录 | 内容 |
| --- | --- |
| [design/](./design/) | 架构总览、模块设计、契约设计 |
| [decisions/](./decisions/) | ADR 与历史决策记录 |
| [plans/](./plans/) | roadmap、后续计划、backlog |
| [solutions/](./solutions/) | 具体功能/问题的实现方案 |
| [reports/](./reports/) | 进度快照、评审/分析/验收报告 |
| [references/](./references/) | 使用指南、外部框架研究、源码地图 |

## 阅读顺序

1. [app-host-contract-onion-adaptation.md](./design/app-host-contract-onion-adaptation.md) — AppHost 契约适配 onion 的差异分析与待决问题（当前讨论中）
2. [apps-monorepo-and-dual-target-evaluation.md](./design/apps-monorepo-and-dual-target-evaluation.md) — apps 目录 + App 单元化 + web/桌面双端支持 方案评估（当前讨论中）
3. [apphost-shell-design.md](./design/apphost-shell-design.md) — 宿主壳布局设计：标题栏/侧栏/tab 主区/画布第二舞台结构、dsh 对齐的设计令牌与控件规格
4. [host-kernel-design.md](./design/host-kernel-design.md) — 宿主内核设计：注册表/事件总线/Agent 通道/装配生命周期的模块职责与不变量
5. [sidecar-lifecycle-design.md](./design/sidecar-lifecycle-design.md) — 边车生命周期设计：双端基址解析、健康检查、App 仓库与安装模型
6. [chatapp-rendering-design.md](./design/chatapp-rendering-design.md) — chatapp 渲染层设计：帧→条目映射、轮次块聚合不变量、审批卡与会话切换（已完成冒烟）
7. [app-dev-standards.md](./references/app-dev-standards.md) — **App 开发规范总纲（开发者入口）**：契约权威链、两种形态、manifest/setup/slot/ctx 编程规则、contributes、安装调试、样式令牌与交付自检
8. [app-dev-guide.md](./references/app-dev-guide.md) — App 开发指南：三方 App 接入的清单/slot/ctx/Agent 通道/安装调试全流程
9. [app-settings-and-multipage-assembly.md](./solutions/app-settings-and-multipage-assembly.md) — App 设置页（config 表单/启停/卸载）与多页面装配规则（页级 config/hostApi、认领表、lazy-cache 键协同）
10. [sessionboard-app.md](./solutions/sessionboard-app.md) — 会话工作台（第二个真实 App）：五类 slot + 五类 inject 面全量狗粮验证，契约观察反哺指南
11. [chatapp-controls.md](./solutions/chatapp-controls.md) — chatapp 会话三控件：工作区/模型/审批模式运行期切换（REST 端点 + AgentChannel + header 控件）
12. [chatapp-slash-goal-plan-todos.md](./solutions/chatapp-slash-goal-plan-todos.md) — chatapp 斜杠命令系统（8 命令，含 /goal、/plan）与 GoalBar / TodoPanel / plan 工具条徽标展示（AgentChannel 六方法 + 严格对齐 dsh 插槽语义，已冒烟）
13. [icon-spec.md](./references/icon-spec.md) — App 图标（AppIcon）设计规范：24×24 网格、描边/填充、单色、尺寸与运行时校验
14. [app-floating-windows-design.md](./design/app-floating-windows-design.md) — App 悬浮舞台设计（浮层式 overlay）：多悬浮窗、拖动/缩放/还原/关闭窗框归壳，App 契约零改动（已完成）
15. [app-settings-secrets-contract-design.md](./design/app-settings-secrets-contract-design.md) — App 配置契约 v3：宿主零策略 + `ctx.registerSettingsPanel` 设置入口机制（已完成）
16. [roadmap.md](./plans/roadmap.md) — 后续工作计划：边车质量债（图片预算管线/compaction 保护）、桌面壳集成、App 生态深化
17. [DESIGN.md](./references/DESIGN.md) — dsh web ui 设计系统提炼（参照系）：色板/字体阶梯/圆角与平滑圆角/阴影三件套/交互节奏与显式禁令
18. [DESIGN.md](./design/DESIGN.md) — **ava2 AppHost 设计系统（唯一设计基准）**：`--ava-*` 令牌契约、HarmonyOS 字体栈、间隔/圆角/阴影阶梯、组件规格、显式禁令与 dsh 差异决策表
19. [chatapp-lazy-session.md](./solutions/chatapp-lazy-session.md) — chatapp 新建会话惰性创建（对齐 dsh）：点新建只进草稿态不落盘、首条消息才创建、空会话过滤与侧边栏草稿展开规则（已冒烟）
20. [tool-approval-workspace-write-fix-plan.md](./solutions/tool-approval-workspace-write-fix-plan.md) — 工具审批修复方案：workspace-write 下基础 shell 命令逐次弹卡的判定链路、六个根因（白名单正向枚举 / shell 无 WorkspaceWrite 档 / OS 沙箱未部署 / 仅 allowed-once / 双次审批 / exec_danger fail-open）与两阶段落地计划
21. [chatapp-attachments.md](./solutions/chatapp-attachments.md) — chatapp 对话图片附件：composer 选择/粘贴 → `send(id, text, attachments)` → REST `file` parts → 气泡缩略图与 durable 回放（仅图片、4MB/6 张上限，真实模型识图 10/10 已冒烟）
22. [desktop-release-and-update.md](./solutions/desktop-release-and-update.md) — 桌面端打包发布与自动更新：版本单一源、publish.ps1 八步（签名 NSIS/MSI）、`ava2/` OSS 前缀隔离布局、update.json 清单与 tauri-plugin-updater 客户端链路

## 维护约定

- 所有文档必须带 YAML Frontmatter（`title` / `description` / `keywords` / `date` 必填）。
- 新增文档按六分类归档；分类判定见 `ava2/.trae/rules/project_rules.md` §3。
