---
title: dsh（DeepSeek Harness）App Host 设计分析
description: dsh 宿主/插件架构的源码实证分析：全插件化服务容器、effect 生命周期、声明即授权的 slot 模型、schema 配置与凭证引用；对照 ava2 AppHost 的借鉴点、反面教材与有意分歧。
keywords: dsh, DeepSeek Harness, 插件宿主, app host, Cordis, 服务容器, slot, ava2 对照
author: AI 助手
date: 2026-11-10
updated: 2026-11-10
status: done
category: references
---

# dsh（DeepSeek Harness）App Host 设计分析

> 来源：`c:\workspace\SuperAI4\deepseek-harness` 源码实证（路径相对 dsh 根）。分析目的：为 ava2 AppHost（定位「宿主即操作系统」）提供参照。

## 1. 总体架构

dsh 不是 GUI 宿主，而是**全插件化的 Cordis Agent Harness**（`AGENTS.md`）：CLI `dsh` 是唯一 Node 应用启动器（`apps/cli/README.md`）。没有特权核心——模型适配器、工具注册表、会话日志、甚至 agent loop 都是插件。

- **扩展单元 = npm 包插件**（`@deepseek-ai/dsh-<name>`），导出 `name` / `inject` / `apply(ctx, config)` / `Config`（`docs/cordis-primer.md`）。
- **打包 = bundle**：Cordis 配置行（patch YAML）+ 挂载代码；`package.json` 的 `dsh.bundle.patch` 指向 patch 文件（`docs/architecture.md`）。
- **发现/组合 = profile**：`$DSH_HOME/profiles/<name>/package.json`（`dsh.profile.bundles` 有序列表）+ `cordis.patch.yml`；启动层叠：bundle 层 → profile patch → home patch → `--patch` overlay（`apps/cli/README.md`）。
- **manifest 不枚举能力面**：只声明 `dsh.manifestVersion` / `bundle` / `profile` / `client` / `engines.dsh` / `platforms`（`packages/util/package-manifest/src/types.ts`），贡献什么完全由代码 `apply()` 决定，**不预先声明 tools/skills/UI 白名单**。

## 2. 插件契约

贡献即「在某个 seam 上注册」：`ctx.tools`（模型工具）、`ctx.commands`（斜杠命令）、`ctx.llm`（模型适配器）、`ctx.skills`、`ctx.mcp`、`ctx.fs` / `ctx.shell` / `ctx.sandbox`、事件监听与 waterfall 拦截器（`docs/architecture.md`）。UI 侧另一面：包声明 `dsh.client` 并导出 `./client`。

生命周期不是 install/enable/disable，而是 **effect 生命周期**：所有注册走 `ctx.effect()` / `ctx.on()`，返回 disposer，插件卸载时自动回溯撤销。加载顺序由 `inject` 服务依赖决定而非手工编排；热重载由 `dsh-hmr` 重组合（`packages/boot/app-boot/README.md`）。显式开关只有 profile 里的 `disabled: true` 行。

## 3. 能力注出与权限收口

注出面就是 **ctx 服务容器**：`ctx.tools` / `ctx.sessions` / `ctx.agents` / `ctx.llm` / `ctx.commands` / `ctx.approval` / `ctx.credentials` / `ctx.slots`(client)。隔离三层：

- **服务级**：`inject` 声明依赖，插件只拿到声明过的服务；`isolate` realm 给 provider+consumer 组隔离域。
- **工具执行级**：`tools/pre-execute` waterfall → 单调 guard → `ctx.approval` 一次性人工批准（缺席即拒绝）→ `tools/execute`（`docs/tool-execution-pipeline.md`）；沙箱 `read-only` / `workspace-write` / `danger-full-access`。
- **版本准入**：装载前校验 `engines.dsh` / peer 范围，不兼容拒绝 + 显式豁免文件（`compatibility.json`）。

**关键反面点**：这些都不是针对恶意包的沙箱——插件 in-process 执行、在工作区沙箱之外（`packages/boot/plugin-manager/README.md` 自认）。

## 4. 配置与密钥（⚠ 与 ava2 v3 决策相反，属有意分歧）

dsh 宿主**统一管理配置且声明式 schema**：插件导出 Schemastery `Config`，宿主 `ctx.settings` 按 profile entry id 投影表单、完整校验、revision 冲突检测，写回 profile patch 文件（`packages/settings/settings/README.md`）；规范强制「插件内无硬编码可调参数」。密钥独立成面：配置只写 `CredentialRef` 名字，值存 `$DSH_HOME/.credentials.yaml`（0600），`role('secret')` 字段表单响应中抹除。

**ava2 有意分歧**：v3 已决策「宿主零策略，配置存储/校验/回显全部由 App 自管」（用户明确否决宿主代渲染/代存储，见 [App 配置契约 v3](../design/app-settings-secrets-contract-design.md)）。dsh 的 schema 配置方案在此**不采纳**；其可借鉴的残余价值仅在「凭证引用而非明文」思想——未来 App 自管密钥时可参考独立存储 + 抹除回显，但宿主默认不参与。

## 5. UI 扩展点

有完整 GUI 宿主：`host/`（HTTP/SPA 服务）+ `client/`（浏览器半边）+ Electron Desktop（IPC 注入）（`packages/host/README.md`）。

- **类型化 slot 注册表** `ctx.slots`：五种组合形态 `single` / `list` / `keyed` / `chain` / `hoc`；含 `settings.section`、`conversation.session.header.actions`、`plugins.detail.badge/section` 等（`packages/client/ui-slots/README.md`、`docs/subsystems/slots.md`）。
- **声明即授权**：只有声明了 child slot 的父组件能渲染它；注册未声明 slot 或重复声明在加载期抛错；`ctx.slots.inject(key, cb)` 贡献随 owner 卸载自动撤销。
- 工具 UI 由 `output.render` 声明（纯 presenter，无 I/O）。
- 无悬浮窗机制——UI 扩展全部是声明位置的 slot 区域，与 ava2 现状同构；ava2 的悬浮窗设计（[app-floating-windows-design.md](../design/app-floating-windows-design.md)）在 dsh 中无对应物。

## 6. 对照 ava2 的结论

**值得吸收**

1. **effect 化注册**：注册统一返回 disposer，宿主卸载时自动回溯——ava2 已是此模型（slot inject 返回 Disposable），可扩展到窗口等新注出面保持一致。
2. **声明即授权的 slot 模型**：ava2 的 slot 目录由宿主声明（`SLOT_KEYS`）、App 按 `surfaces` 声明使用，已与此同构；可补强：声明与实际注册不符时加载期报错。
3. **凭证引用思想**：密钥值独立存储 + 表单抹除——App 自管密钥时的参考模式。
4. **分层 patch + 按行 id 覆盖**：让「用户可改任何配置行」与「宿主零策略」共存的可能路径（ava2 远期若做用户级 App 偏好覆盖）。
5. **required/optional 条目启动矩阵**：必需能力 fail-loud、可选能力 warn-continue。

**反面教材（ava2 不要照抄）**

1. **manifest 无能力面声明**：靠 `apply()` 隐式贡献，「装了插件到底贡献了什么」不可静态审计——ava2 的 `contributes` 声明 + 校验是更优解，保留。
2. **无运行时隔离**：in-process、沙箱外执行、版本检查不是安全边界——ava2 定位 AgentOS，App 加载处应考虑真实边界（worker/子进程 + 能力面 gateway），至少要意识到同源 DOM 下 App 与宿主互信有限。
3. **配置写回文本 patch**：无深合并、无回滚、需写锁 + HMR 串行化——复杂度极高；ava2 v3 让 App 自管存储恰好绕开这套锁。
4. **bundle 准入靠拒绝 + 手工豁免文件**：运维复杂；ava2 的显式 `hostApi` 版本协商更简。
5. **slot 全表线性扫描**（`isLive` 已知限制）：ava2 slot 表已按 App 索引，保持。

**一句结论**：dsh 宿主 = 「服务容器 + 事件总线 + effect 生命周期」，扩展点全部运行时注册。ava2 保留自己的 manifest 声明式契约（这是对 dsh 最大的改进点），吸收 effect 回溯与声明授权模型；配置上坚持 v3 零策略分歧，隔离上警惕 dsh 的「无沙箱」教训。
