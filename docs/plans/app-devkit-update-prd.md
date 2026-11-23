---
title: app-devkit 技能更新 PRD——引入应用开发完整规范并同步 ctx.net 契约
description: 更新 app-devkit 的 app-development 技能与两份参考文档：补齐 ctx.net 统一联网服务契约漂移，并把完整开发规范（app-dev-standards.md）随技能 references 分发，让 Agent 在对话中开发 App 时有完整规范可依。
keywords: app-devkit, app-development, 技能更新, ctx.net, App 契约, 开发规范, PRD
author: AI 助手
date: 2026-11-22
updated: 2026-11-22
status: done
category: plans
---

# app-devkit 技能更新 PRD

## 1. 背景与问题

app-devkit（「App 开发套件」App）通过 `contributes.skills` 向 Agent 注入 `app-development` 技能，是「对话中开发 App」场景下 Agent 的主要知识来源。近期契约演进后，技能与参考文档出现漂移：

| # | 漂移点 | 现状 | 期望 |
| --- | --- | --- | --- |
| P1 | **ctx.net 统一联网服务未覆盖**（今日上线的 hostApi 2 能力面） | SKILL.md §5 与两份文档均写「App 不 fetch 边车以外的地址」，但未给出合法外联出口 | 补齐 `inject: ["net"]` + `manifest.net.allow` 域白名单 + `ctx.net.fetch` 请求/响应形态 + 服务端安全底线 + 越权 fail-loud 语义 |
| P2 | **技能未承载完整规范** | SKILL.md 仅 9.9KB 工作流 + errors.md；完整规范 `docs/references/app-dev-standards.md`（259 行）不在技能包内，Agent 需自行找到仓库路径 | 完整规范随技能 `references/` 分发，SKILL.md 指路；对话开发场景 Agent 一次注入即得全量规范 |
| P3 | **参考实现清单过时** | 「联网型 App」无参考（skills-manager 已是 ctx.net 最佳实践：App 直连读面 + 服务端安装面） | 参考实现表增补 `skills-manager`（ctx.net + installSkillHub） |
| P4 | **manifest 展示字段/示例微漂移** | SKILL.md §4 示例含 `homepage` 字段但缺 `net` 字段示例 | manifest 模板补 `net` 字段 + 白名单写法示例 |
| P5 | **errors.md 覆盖面** | 无 net 相关排查项 | 增补「外联越权」运行期报错的排查说明（属宿主抛错，非 app_* 错误码，单独说明） |

受影响文件（单一来源 + 分发副本共 4 处正文）：

1. `apps/app-devkit/skills/app-development/SKILL.md`（技能主文，v0.2.0 → v0.3.0）
2. `apps/app-devkit/skills/app-development/references/standards.md`（**新增**，完整规范随技能分发）
3. `docs/references/app-dev-standards.md`（规范总纲，权威源）
4. `docs/references/app-dev-guide.md`（叙述版指南）

## 2. 目标与非目标

**目标**

1. Agent 在对话中开发 App 时，经 app-devkit 技能即可获得**与最新契约（hostApi 2 + ctx.net）一致的完整开发规范**，无需额外找文档。
2. 三处正文（SKILL / standards / guide）对 `ctx.net` 的描述与 `contracts/app-contract.json` 完全一致，交付时 `npm run check:contracts` 0 error。
3. 端到端验证：用新技能在对话中成功开发一个联网型 App（热点话题收集），证明规范可用。

**非目标**

- 不改契约 SPEC 与 `src/host/types.ts`（契约已稳定，本次只同步文档）。
- 不做 app-devkit App 的 UI 功能（纯技能 App，无界面改动）。
- 不新增其他能力面的文档重构（storage / events 描述已准确）。

## 3. 方案

### 3.1 SKILL.md v0.3.0（技能主文）

- §1 权威链：第 3 顺位后插入「完整规范（随本技能分发）：`references/standards.md`（权威源 `docs/references/app-dev-standards.md`）」。
- §4 manifest 模板：补 `net` 字段示例（`"net": { "allow": ["api.example.com"] }`），硬性规则补一条「`inject` 声明 `net` 后才可外联；`net.allow` 缺省 = 全部 https 公网，声明即收窄」。
- §5 setup(ctx) 的「平台服务」不变量改写：外联唯一出口为 `ctx.net.fetch({ url, method?, headers?, body?, encoding? }) → { status, contentType, body }`；越权（未声明 `net` 或域不在白名单）宿主直接抛错；App 内禁止自行 `fetch` 第三方站点（CORS 白名单不可控）。
- §5 增「联网服务」小节：方法仅 GET/POST、请求头白名单、响应 ≤4MiB、超时 15s、上游状态码透传、仅 https 公网（拒绝 localhost/私网）。
- §2.4 参考实现表（对应 standards §2.4）：增 `apps/skills-manager`（联网型：ctx.net 读面 + installSkillHub 安装面 + storage 持久化）。
- §6 Gotchas：增「外联 404/405 直接透传上游状态码，按上游语义处理，勿当作宿主错误」。
- §8 自检清单：第 4 条改为「平台服务一律经 ctx：不直连 WS、外联一律走 `ctx.net.fetch`、不用 Tauri 特权」。

### 3.2 references/standards.md（新增，完整规范随技能分发）

- **自包含**（用户确认）：副本内联规范全文，可独立阅读使用，不依赖跳转仓库其他文件才能理解；
- 权威源中的仓库相对链接（`contracts/app-contract.json`、`app-dev-guide.md`、`DESIGN.md`、`icon-spec.md`、`chatapp-rendering-design.md` 等）一律改为**明确的仓库绝对路径文本**（技能安装到 `<ONION_HOME>/apps/` 后相对链接失效，Agent 按路径读文件）；
- 头部注明：本副本自包含、权威源为 `docs/references/app-dev-standards.md`，权威源更新时同步本副本；
- SKILL.md 只指路不重复正文。

### 3.3 权威源与指南同步（docs 两份）

- `app-dev-standards.md`：§5 ctx 能力面表增 `ctx.net` 行；§2.3 manifest 硬性规则补 `net` 条目；§2.4 参考实现表增 skills-manager；§9 自检第 4 条同步改写；`updated: 2026-11-22`。
- `app-dev-guide.md`：§2 manifest 字段表 `inject` 枚举补 `net` + 增 `net` 字段行；§5 ctx 能力面表增 `ctx.net` 行；§10 约束清单同步。

### 3.4 app-devkit manifest

- `version: 0.1.1 → 0.1.2`（技能内容变更，ESM/技能缓存刷新，安装覆盖生效）。

## 4. 验收标准

1. `npm run check:contracts` 0 error / 0 warning。
2. `apps/app-devkit` 重装（overwrite）成功；技能注入验证：新会话中 Agent 可读到 `ctx.net` 规范与 `references/standards.md`。
3. 端到端：ava2 对话中下发「开发一个热点话题收集App」，Agent 按规范走完 读契约 → 开发 → 安装 → 冒烟 全流程，产物符合契约（本 PRD 的最终实测环节）。
4. 三处正文交叉一致：ctx.net 的请求形态 / 白名单语义 / 安全底线三描述一致。

## 5. 交付后动作（本 PRD 之外，用户已指定）

1. 重装 app-devkit → 用户在 App 管理面板体验技能内容，提出修改意见。
2. 启动 ava2，对话中切换创造模式（approvalPreset），下发指令「开发一个热点话题收集App」，观察 Agent 是否按新规范执行。

## 6. 待确认决策点

| # | 决策点 | 结论 | 说明 |
| --- | --- | --- | --- |
| D1 | 完整规范承载方式 | **复制进技能 `references/standards.md`，且必须自包含**（用户确认） | 内联全文 + 仓库文件写绝对路径，不依赖相对链接或二次找文件 |
| D2 | ctx.net 描述详略 | SKILL.md 放「够用规则」（形态/白名单/底线/越权），完整细节留 standards.md | 技能注入成本与信息密度平衡 |
| D3 | 参考实现是否含 App 源码级讲解 | 只列路径 + 一句话定位（Agent 按需读源码） | 避免文档与源码双源漂移 |
