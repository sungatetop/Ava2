---
title: ava2 文档入口
description: ava2（AppHost 桌面壳 + 可插拔 App 契约 + onion-server 边车）文档总入口：架构设计、App 开发指南与设计系统。
keywords: ava2, AppHost, 文档索引, onion-server, Tauri, App 开发
author: AI 助手
date: 2026-11-07
updated: 2026-11-23
status: done
category: references
---

# ava2 文档入口

ava2 = Tauri 桌面壳 + AppHost 可插拔 App 契约 + onion-server Agent 运行时边车。

## 目录结构

| 目录 | 内容 |
| --- | --- |
| [design/](./design/) | 架构总览、模块设计、契约设计 |
| [references/](./references/) | App 开发指南、开发规范、图标规范 |

## 阅读顺序

### 架构设计

1. [host-kernel-design.md](./design/host-kernel-design.md) — 宿主内核：注册表 / 事件总线 / Agent 通道 / 装配生命周期的模块职责与不变量
2. [apphost-shell-design.md](./design/apphost-shell-design.md) — 宿主壳布局：顶部合并条 / 侧栏 / tab 主区 / 画布第二舞台结构与控件规格
3. [sidecar-lifecycle-design.md](./design/sidecar-lifecycle-design.md) — 边车生命周期：进程模型、启动装配、双端基址解析、App 仓库与数据目录
4. [net-service.md](./design/net-service.md) — 统一联网服务（`ctx.net`）：双层安全（服务端安全底线 + App 级域白名单）
5. [app-floating-windows-design.md](./design/app-floating-windows-design.md) — App 悬浮舞台：overlay 层复用 `MainSurface`，App 契约零改动
6. [app-settings-secrets-contract-design.md](./design/app-settings-secrets-contract-design.md) — App 配置契约：宿主零策略 + `ctx.registerSettingsPanel` 设置入口机制
7. [chatapp-rendering-design.md](./design/chatapp-rendering-design.md) — chatapp 渲染层：帧→条目映射、轮次块聚合不变量、审批卡与会话切换、图片上下文预算

### App 开发

8. [app-dev-standards.md](./references/app-dev-standards.md) — **App 开发规范总纲（开发者入口）**：契约权威链、两种形态、manifest/setup/slot/ctx 编程规则、contributes、安装调试、样式令牌与交付自检
9. [app-dev-guide.md](./references/app-dev-guide.md) — App 开发指南：三方 App 接入的清单 / slot / ctx / Agent 通道 / 安装调试全流程
10. [icon-spec.md](./references/icon-spec.md) — App 图标（AppIcon）设计规范：24×24 网格、描边/填充、单色、尺寸与运行时校验

### 设计系统

11. [DESIGN.md](./design/DESIGN.md) — **ava2 AppHost 设计系统（唯一设计基准）**：`--ava-*` 令牌契约、字体栈、间隔/圆角/阴影阶梯、组件规格与显式禁令

## 维护约定

- 所有文档必须带 YAML Frontmatter（`title` / `description` / `keywords` / `date` 必填）。
- 新增文档按 `design/`（架构与契约）与 `references/`（开发指南与规范）两类归档。
