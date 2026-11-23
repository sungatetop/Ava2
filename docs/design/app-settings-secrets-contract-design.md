---
title: App 配置契约 v3：宿主零策略 + 设置入口机制
description: ava2 AgentOS 的 App 配置最终决策（v3）：宿主只提供设置入口机制，配置的表单/存储/校验全部由 App 自行决定；记录 v1/v2 方案被否的历史与迁移清单。
keywords: ava2, App 契约, settings, 宿主零策略, 设置面板, AgentOS, apps 市场
author: AI 助手
date: 2026-11-10
updated: 2026-11-10
status: done
category: design
---

# App 配置契约 v3：宿主零策略 + 设置入口机制

## 1. 最终决策（v3）

> 用户原话：「app host 不做任何额外处理，配置如何存储，也是由 app 开发者决定。app host 相当于操作系统。」

**宿主零策略**：AppHost 对 App 配置不做任何存储、校验、回显、徽标、引导处理。宿主只提供 OS 式「机制」——设置入口（类比 Android 的 settings activity）：

| 层 | 职责 |
| --- | --- |
| 宿主（机制） | 详情页「设置」按钮 + 弹层惰性挂载 App 注册的组件 + ErrorBoundary 隔离 |
| App（策略） | 设置面板 UI、表单控件、配置存储（同源 localStorage / ctx.storage / fetch 均可）、校验、是否回显（如密码框自带的显示/隐藏切换） |

## 2. 机制实现

- **登记**：`ctx.registerSettingsPanel(factory)`（[types.ts](file:///c:/workspace/SuperAI4/ava2/src/host/types.ts) `SettingsPanelFactory`）。App 在 `setup(ctx)` 期调用，工厂与 slot 注入同一模式：`() => Promise<{ default: ComponentType }>`。
- **注出不受限**：登记是纯声明（类比 manifest 自描述），不占能力面、不进 `inject`；未登记 = 详情页无「设置」按钮（与 suspended App 的 slot 贡献不可见同一逻辑）。
- **挂载**：应用市场详情页出现「设置」按钮 → [AppSettingsModal](file:///c:/workspace/SuperAI4/ava2/src/host/AppHost.tsx) 弹层经 `lazy-cache` 键控缓存（`<appId>:"settings"`）惰性挂载，ErrorBoundary + Suspense 隔离。
- **生命周期**：工厂存于 registry 的 App 记录，卸载/重装随记录一并消失；设置弹层随卸载关闭。

## 3. 被否的方案（历史）

| 版本 | 方案 | 否决原因 |
| --- | --- | --- |
| v1 | `manifest.config`（JSON Schema 子集）+ 宿主 `AppSettingsForm` 统一渲染 + localStorage `app:<id>:config` 统一存储 + `ctx.settings` 注出 | 宿主代渲染/代存储违反零策略；复杂表单表达力不足 |
| v2 | v1 + secret 三分类（preference/connection/secret）+ `format:"secret"` 掩码 + required 徽标 + 未配置引导 | 密钥特殊逻辑仍归宿主；「是否回显由 UI 决定」（用户纠正）意味着宿主无需感知 secret |

v2 曾引入的 `InjectKey:"settings"`、`ConfigSchema/ConfigPropertySchema/ConfigValues/AppSettings`、`config.ts`（readRawConfig/writeConfig）、manifest config 校验、待配置徽标、卸载清配置——**全部已删除，不得恢复**。

## 4. 迁移清单（本次落地）

- 宿主：删 `config.ts` / config 契约类型 / `settings` 注出（`INJECT_KEYS` 收窄为 slots/events/agent/storage）/ `AppSettingsForm` / 徽标与卸载清配置；新增 `registerSettingsPanel` + `AppSettingsModal`。
- demo App：task-manager 0.1.1、model-manager 0.1.1、sessionboard 0.1.4——`ctx.settings.get` → 自管 `readCfg`（localStorage 自定键），setup 登记 `SettingsPanel`（confirmDelete / pageSize 自校验）。
- chatapp 0.9.6：新增 `src/settings.tsx`（昵称/强调色演示字段，自存 `chatapp:cfg`），`registerSettingsPanel(() => import("./settings"))`。
- 文档：app-devkit SKILL.md 同步零策略约定。
- 受 `INJECT_KEYS` 收窄影响的旧安装 App 需 overwrite 重装。

## 5. 后续空间（不承诺）

- 需要跨端同步/密钥安全时，由 App 自行调用边车接口或引入平台级可选服务，宿主默认不参与。
- 悬停未装配 App 的「设置」入口：如未来需要（类比 Android 冷启动 settings activity），可扩展为 manifest 静态声明 + 按需装配，当前不做。
