---
title: App 运行态设置与多页面装配深化方案
description: 多页面 plugin 装配规则深化（页面注册归一、页级 hostApi 生效、认领表与 lazy-cache 键协同）的实现方案；其中「宿主代管 config 设置页」部分已被配置契约 v3（宿主零策略）否决取代，仅作历史记录。
keywords: AppHost, 设置页, config schema, 多页面, contributes.apps, 认领表, lazy-cache, 配置契约v3
author: AI 助手
date: 2026-11-08
updated: 2026-11-11
status: archived
category: solutions
---

# App 运行态设置与多页面装配深化方案

> 对应 roadmap 2.2（`docs/plans/roadmap.md`）。两项工作同属「App 生态」装配链路，合并一轮落地。

> **历史注记（2026-11-11）**：本文「运行态设置」一节的宿主代管方案（config schema 表单渲染、`ctx.settings` 注入、徽标/引导）已被配置契约 v3 否决并移除——宿主零策略，配置的表单/存储/校验全归 App，宿主只提供 `ctx.registerSettingsPanel` 设置入口机制，见 [app-settings-secrets-contract-design.md](../design/app-settings-secrets-contract-design.md)。**多页面装配与缓存一致性部分仍然有效**。

## 1. 背景与目标

- **运行态设置**：enable/disable 已有 REST 面（`PUT /v1/apps/{id}/enabled`，边车登记表持久化），App 管理视图（`__apps`）也已提供启停/卸载按钮；但声明式配置（`manifest.config`）只有读取链路——`writeConfig`/`readConfig` 已实现而**无任何 UI 调用**，`app-dev-guide.md` 承诺的「宿主设置页渲染表单，值经 `ctx.settings` 注入」未兑现。
- **多页面装配**：`contributes.apps[]` 多页声明已被 `resolvePages` 归一、`makeContext` 已带 `pageId`，但两处页级字段解析后被丢弃：页级 `config`（每页 schema 覆盖）与页级 `hostApi`（每页契约版本）均未生效，`registry.load` 只按 plugin 级 schema 校验一次。
- **缓存一致性**：卸载 App 时 lazy-cache 未清理，同 id 覆盖重装（`overwrite=true`）后会复用旧 lazy 实例。

## 2. 设计决策

### 2.1 设置页（App 管理视图内联展开，不新增独立视图）

- **入口**：App 管理视图每个 App 条目内新增「设置」按钮，仅当该 App 存在有效配置 schema 时显示；点击在条目下方展开内联表单（不新增侧栏导航项——配置是 App 级而非平台级，内联展开保持「一个 App 一张卡片」的心智）。
- **schema 取值**：`resolvePages(manifest)[0]?.config ?? manifest.config`——与首页实际注出 `ctx.settings` 的 schema 同源。多页 App 以首页 schema 为编辑面；纯页级声明（plugin 级未声明）也可编辑。
- **控件映射**（JSON Schema 子集 → 表单控件）：

| schema | 控件 | 空值行为 |
| --- | --- | --- |
| `enum` 非空 | `<select>`（选项为枚举值） | 未设置回落 `default` |
| `boolean` | `<checkbox>` | 未设置回落 `default ?? false` |
| `number` | `<input type=number>` | 清空 = 移除该键（回落 `default`） |
| `string` | `<input type=text>` | 空串视为合法值存 `""`；未设置回落 `default`（placeholder 展示） |

- **保存链路**：`writeConfig(manifest, values, schema)`（按 schema 校验，非法值抛错不落盘）→ App 已装配时 `clearLazyCache(id)` + `registry.reload(manifest)`（重新 setup 使新值经 `ctx.settings` 注出）；suspended 态只写值不重载。`config.ts` 的 `writeConfig` 增加可选 schema 参数（plugin 级为缺省），签名向后兼容。
- **运行态设置收口**：启停/卸载/配置三项在同一卡片内完成，权威均在边车登记表（启停持久化）与 localStorage（配置值），宿主只做实例装配与排空。

### 2.2 多页面装配规则深化

**页面注册归一**（已有，规则固化）：`contributes.apps[]` 非空 → 逐页，页级字段缺省继承 plugin 级（`hostApi`/`inject`/`surfaces`/`config`）；否则顶层 `entry` 视为唯一主页（`pageId = "."`）。`parseManifest` 的旧形态归一（顶层 `apps` 字段映射进 `contributes`）继续兜底。

**页级 schema 生效**（本轮）：`registry.load` 改为逐页校验——每页用 `page.config ?? manifest.config` 校验同一份存储值（`app:<id>:config`，plugin 级一份存储、按页 schema 过滤注出），任一页校验失败 → 整个 App failed（fail-loud，阻止后续页 import）。纯能力 App（无 UI 页）保持 plugin 级校验兜底。

**页级 hostApi 生效**（本轮）：`makeContext` 增加页级契约版本参数，能力面 granted 判定用 `pageHostApi ?? manifest.hostApi`——plugin `hostApi=0`（旧契约全量）但某页显式声明 `hostApi=1` 时，该页按显式注出收窄。

**认领表**（已有，规则固化）：

- 认领型 slot（`surface.main`/`surface.canvas`）的 `viewId` 即路由键，跨 App 全局唯一；同一 App 可认领多个不同 viewId（多页 App 各页认领一个主视图是合法形态）。
- 冲突 fail-loud：后认领者 `inject` 抛错 → 该 App failed，先到者不受影响（并行 load 隔离）。
- 回收：`unload`/`failed` 时 `uninjectAll` 同步回收认领，避免永久占住 viewId；同一 App 对同 viewId 注入多次时最后一个撤回才释放。

**lazy-cache 键协同**（规则表）：

| 舞台 | 缓存键 | 说明 |
| --- | --- | --- |
| 主区 | `appId::viewId` | 同 App 多页贡献靠 viewId 区分 |
| 画布 | `appId::canvas:<viewId>` | 与主区隔离，同 App 可两舞台并存 |
| 侧栏面板 | `appId::<viewId>:sidebar` | 面板 viewId 是对主区视图的引用，不参与认领 |
| 画布面板 | `appId::canvaspanel:<viewId>` | 同上 |

清理时机（本轮补齐）：**卸载**与**配置保存后重载**必须 `clearLazyCache(appId)`——卸载后同 id 重装会换产物版本，复用旧 lazy 实例会跑旧模块；禁用（suspend）不清（模块保留，恢复零成本）。

## 3. 变更清单

| 文件 | 变更 |
| --- | --- |
| `src/host/registry.ts` | `load` 逐页 schema 校验（页级 config 生效）；`makeContext` 增加页级 hostApi 参数 |
| `src/host/config.ts` | `writeConfig` 增加可选 schema 参数 |
| `src/host/AppHost.tsx` | AppList 增加设置展开区与 `AppSettingsForm`；卸载/保存重载前清 lazy-cache |
| `src/App.css` | `.app-settings` 表单样式（消费既有 design token） |

## 4. 验收

`tsc --noEmit` 通过；dev.ps1 起环境后以 CDP 驱动 headless Edge 在 `http://localhost:1420/` 完成七步真实交互冒烟（脚本 `.ava2-smoke/cdp-smoke.mjs`，不入库），全部通过：

1. **多页装配**：双页 App（页甲继承 plugin 级 schema、页乙页级 schema 覆盖）安装后两页导航（多页·甲/乙）同时出现——同 App 两页各自认领不同 `surface.main` viewId，认领表工作正常；
2. **设置表单**：App 管理卡片「设置」按钮展开表单，按 schema 渲染 string/number 字段（title/description/placeholder=default）；
3. **越界校验**：`volume=120`（> maximum 100）保存被拦截并显示「配置非法，未保存」——schema minimum/maximum 生效；
4. **保存→重载**：修正后保存，localStorage 落盘 + `registry.reload` 重跑 setup，提示「已保存并重新装配」存活（提示由 AppList 持有，跨 remount 存活，见 §2.1）；
5. **页甲新值注出**：切到页甲视图，`ctx.settings` 注出保存后的值（问候语「你好，设置页」/ 音量 80）——保存生效；
6. **页乙页级覆盖**：页乙视图显示 `pageId: b`、阈值缺省 3、绿色「✓ 注出已按页 schema 过滤（无 greeting/volume）」——页级 config 覆盖与按页过滤生效；
7. **回归**：chatapp 历史会话与 hello 契约冒烟卡片渲染正常，不受装配改动影响。

**冒烟中发现并修复的真实缺陷**：保存成功提示原本是表单组件本地 state——reload 使 `listApps` 瞬时少一项、`<li>` 卸载重挂，本地 state 丢失导致保存反馈不可见。已上抛 AppList 持有。
