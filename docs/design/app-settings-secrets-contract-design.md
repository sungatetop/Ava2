---
title: App 配置契约：宿主零策略 + 设置入口机制
description: ava2 的 App 配置契约：宿主只提供设置入口机制，配置的表单/存储/校验全部由 App 自行决定。
keywords: ava2, App 契约, settings, 宿主零策略, 设置面板, apps 市场
author: AI 助手
date: 2026-11-10
updated: 2026-11-23
status: done
category: design
---

# App 配置契约：宿主零策略 + 设置入口机制

## 1. 核心决策

**宿主零策略**：AppHost 对 App 配置不做任何存储、校验、回显、徽标、引导处理。宿主只提供 OS 式「机制」——设置入口（类比 Android 的 settings activity）：

| 层 | 职责 |
| --- | --- |
| 宿主（机制） | 详情页「设置」按钮 + 弹层惰性挂载 App 注册的组件 + ErrorBoundary 隔离 |
| App（策略） | 设置面板 UI、表单控件、配置存储（同源 localStorage / ctx.storage / fetch 均可）、校验、是否回显（如密码框自带的显示/隐藏切换） |

## 2. 机制实现

- **登记**：`ctx.registerSettingsPanel(factory)`（`src/host/types.ts` 的 `SettingsPanelFactory`）。App 在 `setup(ctx)` 期调用，工厂与 slot 注入同一模式：`() => Promise<{ default: ComponentType }>`。
- **注出不受限**：登记是纯声明（类比 manifest 自描述），不占能力面、不进 `inject`；未登记 = 详情页无「设置」按钮（与 suspended App 的 slot 贡献不可见同一逻辑）。
- **挂载**：应用市场详情页出现「设置」按钮 → `AppSettingsModal` 弹层经 `lazy-cache` 键控缓存（`<appId>:"settings"`）惰性挂载，ErrorBoundary + Suspense 隔离。
- **生命周期**：工厂存于 registry 的 App 记录，卸载/重装随记录一并消失；设置弹层随卸载关闭。

## 3. 约束

- 宿主不引入 `manifest.config` schema、不代渲染表单、不代存储配置、不感知 secret 分类——这些全部归 App。
- 宿主不提供 `ctx.settings` 注出面；App 需要配置时自行经 `ctx.storage` 或自有后端读写。

## 4. 后续空间（不承诺）

- 需要跨端同步/密钥安全时，由 App 自行调用边车接口或引入平台级可选服务，宿主默认不参与。
- 悬停未装配 App 的「设置」入口：如未来需要（类比 Android 冷启动 settings activity），可扩展为 manifest 静态声明 + 按需装配，当前不做。
