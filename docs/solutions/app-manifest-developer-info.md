---
title: App 契约新增开发者信息与图标（author / homepage / license / icon）方案
description: 在 App manifest 契约中新增四个可选展示字段——author（开发者）、homepage（主页链接）、license（开源协议）、icon（应用图标声明位），覆盖 SPEC、类型权威、防漂移校验、图标解析优先级、UI 展示与文档的完整改动链，hostApi 不升级、向后兼容。
keywords: app-contract, manifest, author, homepage, license, icon, 开发者信息, 契约演进, ava2
author: AI 助手
date: 2026-11-11
updated: 2026-11-11
status: done
category: solutions
---

# App 契约新增开发者信息与图标（author / homepage / license / icon）方案

## 1. 背景与目标

当前 manifest 契约的可展示字段只有 `name` / `description`（见 `src/host/types.ts` 的 `AppManifest`），存在两个缺口：

1. **无开发者信息**：应用详情面板无法标识「这个 App 是谁开发的、哪里了解更多」。
2. **无图标声明位**：图标目前由 `registry.appIconFor()` 在 App 注入的 slot spec 里运行时捞取（nav.sidebar → surface.canvas → panel.canvas，见 [registry.ts](file:///c:/workspace/SuperAI4/ava2/src/host/registry.ts#L271-L281)）。纯能力 App（mcp-echo、app-devkit 等无 UI slot）在应用宫格中**永远没有图标**；图标语义上属于 App 元信息，却寄生在 slot 贡献里。

**目标**：manifest 新增四个可选展示字段，全链路（机器可读 SPEC → 类型权威 → 校验器 → UI 展示 → 文档）一次打通，旧 App 零改动兼容。

**非目标**（明确不做，留待后续演进）：

* 开发者签名 / 身份认证（需密钥体系与分发渠道，属信任体系专题）
* 多开发者 / 组织层级、manifest 级结构化 `developer` / `repository` 等扩展字段
* 位图图标资源（png/svg 文件）加载机制——icon 沿用现有 `AppIcon`（内联 SVG path，见 `docs/references/icon-spec.md`），无新资源管道

## 2. 字段设计

### 2.1 四个可选字段（平铺，与 description 同级）

```jsonc
// manifest.json 示例
{
  "id": "task-manager",
  "name": "定时任务",
  "version": "0.2.0",
  "hostApi": 2,
  "description": "管理会话级定时任务",
  "author": "Ava 平台组",                      // ← 开发者/团队展示名
  "homepage": "https://example.com/task-manager", // ← 主页/仓库 URL（详情面板可点击）
  "license": "MIT",                             // ← 开源协议（SPDX 标识符）
  "icon": { "d": "M12 6v6l4 2", "fill": false }   // ← 应用图标（AppIcon，24×24 SVG path）
}
```

| 字段         | 类型                                                    | 语义                                                                                         |
| ---------- | ----------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `author`   | `string`                                              | 开发者/团队展示名，详情面板「开发者：xxx」                                                                    |
| `homepage` | `string`                                              | 主页/仓库 URL，详情面板可点击链接；UI 仅对 `http(s)://` 前缀渲染为 `<a>`，其余值按纯文本防注入                              |
| `license`  | `string`                                              | 开源协议，推荐 SPDX 标识符（如 `MIT` / `Apache-2.0`），详情面板「协议：xxx」；与 author 合并为一行展示（「开发者：xxx · 协议：MIT」） |
| `icon`     | `AppIcon`（复用现有类型：`string` = 单 path d，或 `{ d, fill }`） | 应用展示图标，见 §3 icon 解析语义                                                                      |

选型对比（author 部分）：

| 方案                                       | 例        | 取舍                                       |
| ---------------------------------------- | -------- | ---------------------------------------- |
| **平铺 `author` + `homepage`（推荐）**         | 两个一级可选字段 | 与 `description` 风格一致；两信息相互独立、缺谁都可用；改动面最小 |
| 结构化 `developer?: { name, url, contact }` | 嵌套对象     | contact 无展示位；校验与文档成本翻倍；未来确需时再演进（§2.3）    |

### 2.2 icon 解析语义（契约修复的核心）

`icon` 是**声明式权威来源**，解析优先级调整为：

```
manifest.icon（静态声明，安装期即可用）
  ?? slot 遍历（nav.sidebar → surface.canvas → panel.canvas，现行为降级为兜底）
  ?? undefined（AppIconView 渲染默认占位）
```

理由：图标属于 App 元信息，manifest 是唯一静态权威——纯能力 App、禁用态 App、slot 未加载前的宫格渲染，都应能拿到图标；slot 内 `icon` 保留（导航等局部场景各自渲染），不再承担「应用形象」的寄生职责。

### 2.3 演进路径（预留而非实现）

若未来需要结构化开发者（多字段、认证标识），新增 `developer` 对象字段，`author` 保留为兼容别名（读取顺序 `developer?.name ?? author`）。`repository` 等同理按需追加（`license` 已随本次落地）。本次仅在契约文档「演进预留」一节记录。

## 3. 兼容性分析

| 关注点                                                 | 结论                                                                                                                     |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| hostApi 版本                                          | **不升级**。三个可选展示字段与 `description` 同级，缺省语义完全兼容，无行为分叉                                                                      |
| 旧 App（未带新字段）                                        | 零影响：字段全部可选，UI 缺省不渲染；icon 回落 slot 遍历（现行为不变）                                                                             |
| 带 slot icon 的 App（chatapp / task-manager / file-\*） | 行为不变：manifest.icon 未声明时仍走 slot 遍历；示范迁移后图标来源切换为 manifest，展示结果一致                                                         |
| 边车（onion-server）                                    | `app_contrib.rs` 以 `serde_json::Value` 解析 manifest 并透传，**加字段零代码改动**；仅需落地时确认 `rest/apps.rs` 的安装/发现响应无字段白名单过滤            |
| 契约校验器                                               | `check-app-contract.mjs` 按 `contracts/app-contract.json` 的 `manifest.fields` 交叉校验，SPEC 增补后自动放行（若存在未知字段拒绝逻辑，SPEC 已先行覆盖） |

## 4. 改动清单

按「契约单一来源」约定（`app-dev-guide.md`：改契约先改 SPEC，再同步实现）：

| # | 文件                                                  | 改动                                                                                                                                                                             |
| - | --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1 | `contracts/app-contract.json`                       | `manifest.fields` 追加三项：`author`（string / optional / minLength 1）、`homepage`（string / optional，描述注明须为 http(s) URL）、`icon`（AppIcon / optional，引用 icon-spec 约束）                   |
| 2 | `src/host/types.ts`                                 | `AppManifest` 追加 `author?: string` / `homepage?: string` / `icon?: AppIcon` 三字段注释声明（AppIcon 已在本文件定义，直接复用）                                                                      |
| 3 | `scripts/check-app-contract.mjs`                    | 确认字段校验逻辑从 SPEC 驱动、无需硬编码改动；若存在 manifest 字段白名单（拒绝未知键），确认 SPEC 增补后即放行                                                                                                             |
| 4 | `src/host/registry.ts`                              | `appIconFor` 首选 `this.apps.get(appId)?.manifest.icon`，未声明再走现有 slot 遍历（兜底顺序不变）                                                                                                  |
| 5 | `src/host/AppHost.tsx`                              | 详情面板（L458-460 drawer-desc 处）追加两行：`开发者：{author}`；`homepage` 以 `<a href>` 渲染（仅 http(s) 前缀，`target="_blank" rel="noreferrer"`）。宫格 tile 不加（保持紧凑）；复用现有样式类，无新 CSS                    |
| 6 | `apps/*/manifest.json`                              | 示范迁移：`task-manager` 补 `icon`（现 nav.sidebar 的 clock path 挪入 manifest，slot 内 icon 保留）+ `author` + `homepage`；`chatapp` 补 `author` / `homepage`（icon 已有 slot，验证兜底路径）；其余预置 App 不强制 |
| 7 | `docs/references/app-dev-guide.md`                  | manifest 示例与字段说明表补三行；icon 段落注明「manifest.icon 为应用形象权威声明，slot icon 为局部导航/面板场景」                                                                                                   |
| 8 | `docs/design/app-host-contract-onion-adaptation.md` | 「演进预留」记录：author/homepage 为展示级可选字段；结构化 developer 对象与 repository/license 为未来扩展位                                                                                                  |

## 5. 验证方案

1. **契约防漂移**：`npm run check:contracts`——SPEC ↔ `types.ts` ↔ apps manifest 三方一致
2. **类型门禁**：`npx tsc --noEmit`（ava2 根）
3. **浏览器实测**（真实 API，dev 环境 Vite 热重载即生效）：
   * 应用宫格：task-manager 图标来自 manifest.icon（与迁移前一致）；mcp-echo / app-devkit（纯能力 App）若未声明 icon 仍为占位
   * 详情面板：带 author/homepage 的 App 显示「开发者：xxx」与可点击主页链接；不带的 App 两行不渲染
4. 边车若确认零改动则无需重启；registry/AppHost 为前端改动，Vite 热重载直接生效

## 6. 工作量

改动总量 < 100 行（含文档），单次可评审落地，无需拆阶段。

## 7. 落地记录（2026-11-11）

* `license` 字段按用户补充需求加入（方案评审时由「演进预留」提级为本次落地），SPEC / types.ts / UI / 示范 manifest / 文档五处同步；`repository` 与结构化 `developer` 仍留演进预留。
* 校验器补 `AppIcon` 类型分支：`check-app-contract.mjs` 的字段类型映射表原先无 `AppIcon`，manifest 声明 `icon` 会被误判类型错误，已按 `string | { d, fill }` 形态补齐。
* **顺带修复既有 SPEC 漂移**（types.ts 为类型权威，此前多轮功能演进未同步 SPEC，导致 `check:contracts` 基线即红）：
  * `enums.inject` 去掉已不存在的 `settings`（实现 `InjectKey` 仅四键）；
  * `enums.slotKeys` / `slots` 补 `user.menu`（用户菜单功能，commit ac459f7）；
  * `ctx.surfaces` 对齐实现（`settings` → `registerSettingsPanel`，notes 同步）；
  * `manifest.fields` 移除已废弃的 `config`（实现无消费、无 App 声明）。
* 构建型 App（chatapp / task-manager）的 `dist/manifest.json` 随源码清单同步复制。
* 验证：`npm run check:contracts` 0 error / 0 warning；`npx tsc --noEmit` 通过；浏览器实测见 §5。
