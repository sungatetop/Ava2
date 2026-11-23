---
title: skills-manager App 与技能启停机制设计方案
description: 分析 onion 技能注入链路现状，设计工作区级技能禁用列表（.onion/skills-manager.json）与 skills-manager 管理应用的三层方案。
keywords: skills-manager, skill registry, catalog 注入, REST, AppHost, 启停管理
author: AI 助手
date: 2026-11-21
updated: 2026-11-22
status: draft
category: solutions
---

# skills-manager App 与技能启停机制设计方案

## 1. 背景与目标

为 AppHost 新增一个 **skills-manager** 应用，管理系统与用户工作空间中的技能（skill），
支持启用/禁用管理：

- 禁用列表落盘于 `<projectRoot>/.onion/skills-manager.json`（随仓库走，可提交共享）；
- 凡在 `disabled` 列表中的技能，**不自动注入系统提示词**（目录）；
- 同步约束手势（`/name`）与 `skill` 工具调用，保证目录与调用面一致；
- 管理界面实时生效，无需重启会话。

## 2. 现状分析

### 2.1 技能注入链路（不是静态 system_prompt，而是瀑布 listener）

| 消费点 | 位置 | 行为 |
| --- | --- | --- |
| **目录注入** | `crates/onion/src/skill/catalog.rs` `SkillCatalogListener::pre_step`（L70-167） | 每步 pre-stage 调 `registry.list(cwd)`，过滤 model-invocable 后渲染 `<available_skills>` 目录消息；digest（sha256 of entries）去重，变更时发 `update: true` 替换消息，全空发 tombstone |
| **手势注入** | `crates/onion/src/skill/gesture.rs` `SkillGestureListener::pre_step`（L52-99） | 扫描用户输入 `/name` token，`registry.get(name, cwd)` 命中且 user-invocable 则注入 `<skill_content>`；未命中则按普通 prose 放过 |
| **skill 工具** | `crates/onion/src/skill/tools.rs`（L82-115） | 校验名 → `registry.get` → 检查 model-invocable → 渲染内容 |
| **registry 合并** | `crates/onion/src/skill/registry.rs`（L176-249） | 各 provider 收集候选，按 `rank → provider_order → local_order` 排序，同名 first-wins |

三个消费点全部经过 `SkillRegistry::list / get` —— **registry 是唯一可收敛的过滤接缝**。

### 2.2 REST 与 App 通道现状

- `GET /v1/skills?session=<id>`（`rest/capabilities.rs` L158-172）：从 live 会话
  `handle.skill_registry.list(session_cwd)` 取合并列表；DTO `SkillInfo`
  （`rest/dto.rs` L688-699）已含 `name/description/source/provider/path/modelInvocable/userInvocable`，**无 enabled 字段**。
- 会话句柄可直接拿到 `session_cwd`（项目根解析的数据源）。
- AppHost 应用通过 `ctx.agent`（`AgentChannel`，`src/host/types.ts` L553+）调用服务端，
  方法即 REST 封装（如 model-manager 的 `listModels/addModel/...`）；应用自管 UI 配置存
  localStorage，服务端数据一律走 REST（model-manager 是最贴近的样板）。
- App 通过 manifest 声明 slot 注入（`nav.sidebar` / `surface.main`），`ctx.registerSettingsPanel`
  注册设置面板。
- **当前没有任何技能级启停机制**：仅有 per-skill frontmatter
  `disable-model-invocation` / `user-invocable`（技能作者控制），没有使用者视角的全局开关。

### 2.3 结论

- 过滤器放在 **registry 内部**（list/get 双钩子）即可同时覆盖目录、手势、工具三个消费点，
  避免三处分别实现；
- 目录 listener 的 digest/update 机制天然支持「中途启停 → 下一步发布目录替换消息」，
  无需新增失效通知链路；
- REST 只需一对读写端点 + `SkillInfo.enabled` 字段；应用侧照抄 model-manager 骨架。

## 3. 方案总览（三层）

```
┌─ apps/skills-manager（新增 App）────────────────────────┐
│  技能列表（按来源分组）+ 启停开关 + 禁用文件路径展示      │
│  ctx.agent.listSkills / getSkillManager / putSkillManager│
└──────────────┬──────────────────────────────────────────┘
               │ AgentChannel（宿主 src/host 实现，REST 封装）
┌──────────────▼──────────────────────────────────────────┐
│  onion-server REST                                       │
│  GET/PUT /v1/skills-manager?session=<id>                 │
│  GET /v1/skills?session=<id>（+ enabled 字段）            │
│  读写 <projectRoot>/.onion/skills-manager.json（原子写）  │
└──────────────┬──────────────────────────────────────────┘
               │ 共享 Arc<Mutex<SkillRegistry>>
┌──────────────▼──────────────────────────────────────────┐
│  onion 内核：skill/disable.rs + registry 过滤钩子         │
│  list(cwd) / get(name, cwd) 逐调用解析禁用集，目录/手势/   │
│  工具三消费点自动收敛                                      │
└──────────────────────────────────────────────────────────┘
```

## 4. 详细设计

### 4.1 skills-manager.json 契约

- **路径**：`<projectRoot>/.onion/skills-manager.json`。projectRoot 复用技能发现同款
  `find_project_root`（自会话 cwd 向上找 `.git`，找不到落 cwd 本身），与 `.onion/skills`
  根目录约定对齐。
- **Schema**：

```json
{
  "disabled": ["commit-helper", "pdf-reader"]
}
```

- **读取语义（fail-open）**：文件不存在 → 空集；JSON 解析失败 / 字段类型不符 →
  视为空集并 `warn!` 日志（不阻断会话）；未知字段忽略；名字去重、非 kebab-case 名忽略。
- **写入语义**：tmp + rename 原子写；PUT 全量替换（与文件语义一致，无并发合并）；
  `.onion` 目录不存在则创建。
- **匹配键**：技能名（registry 合并本身按名去重，名字即身份）。禁用名字与来源无关：
  project / user / claude / dsh / agents 任意来源同名技能一并禁用；
  runtime（App contributes 注册）技能同样可被禁用。

### 4.2 内核：`skill/disable.rs` + registry 过滤钩子

新增模块 `crates/onion/src/skill/disable.rs`（目标 < 200 LoC，含 `disable_tests.rs`）：

```rust
/// 禁用策略：按 cwd 解析当前生效的禁用名集合（每次调用重读，保证外部编辑即时生效）。
pub trait DisablePolicy: Send + Sync {
    fn disabled_names(&self, cwd: Option<&Path>) -> HashSet<String>;
}

/// 文件实现：读 `<projectRoot>/.onion/skills-manager.json`（find_project_root 复用）。
pub struct FileDisablePolicy { /* 无状态；路径全部由 cwd 派生 */ }
```

- **registry 接线**：`SkillRegistry` 增加 `with_disable_policy(Arc<dyn DisablePolicy>)`；
  `list()` 在排序去重后过滤禁用名；`get(name)` 命中禁用集时返回 `None`。
- **逐调用重读**：文件极小（几十个名字），list/get 在每个 pre-step 才被调用一次，
  每次 `fs::read` 开销可忽略；换来「外部手改文件 / App PUT 后下一轮立即生效」，
  免去缓存失效通知。
- **默认无策略**：未接线时行为与现状完全一致（既有测试零改动）。
- 装配点：`assemble.rs` 构造 registry 处（L738-750 附近）挂 `FileDisablePolicy`。

### 4.3 三个消费点的语义效果

| 消费点 | 禁用后行为 | 说明 |
| --- | --- | --- |
| 目录（catalog） | 从 `<available_skills>` 消失 | digest 变化 → 下一 pre-step 自动发布 `update: true` 替换目录；全部禁用时发空 tombstone（现有逻辑） |
| 手势（`/name`） | `get` 返回 None → 手势不被认领，按普通 prose 处理 | 与未知名行为一致 |
| `skill` 工具 | `get` 返回 None → 报「unknown skill」类错误 | 模型不会主动调用（目录里没有），防御性兜底 |
| 会话恢复 | 恢复后按当前文件状态注入目录 | 历史目录消息仍在 transcript，靠 digest 对比判定首次/替换（现有机制） |

即「不自动注入系统提示词」由目录过滤直接满足；手势与工具同步收敛是为避免
「目录看不到、却能 `/name` 强行唤起」的不一致语义。

### 4.4 REST 端点（onion-server）

新增模块 `crates/onion-server/src/rest/skills_manager.rs`（避免 capabilities.rs 膨胀），
在 `rest/mod.rs` 注册：

| 端点 | 语义 |
| --- | --- |
| `GET /v1/skills-manager?session=<id>` | `{ disabled: string[], file: "<abs path>" }`（读文件，fail-open 返回空集） |
| `PUT /v1/skills-manager?session=<id>` | body `{ disabled: string[] }`；校验 kebab-case + 去重排序；原子写；返回同 GET 形状 |

- `GET /v1/skills?session=<id>` 的 `SkillInfo` 增加 `enabled: bool`
  （`from_summary` 处按同一 policy 计算），应用列表直接呈现；
- 两个 handler 均经 `require_session` 取 live 会话（鉴权中间件复用现有 `X-Onion-Token` 链路），
  project root 由 `session.handle.session_cwd.current()` 派生；
- 注意 `disabled` 名单不影响 `GET /v1/skills` 的列表返回（管理界面需要看到全部技能，
  包括被禁用的），只影响 `enabled` 字段取值。

### 4.5 宿主 AgentChannel 扩展（src/host）

`src/host/types.ts` `AgentChannel` 增加：

```ts
/** 技能启停：读取/写入工作区 <projectRoot>/.onion/skills-manager.json。 */
getSkillManager(sessionId?: string): Promise<{ disabled: string[]; file: string }>;
putSkillManager(disabled: string[], sessionId?: string): Promise<{ disabled: string[]; file: string }>;
```

宿主实现（`AgentChannel` 的 REST 封装处）补两个 fetch 方法；无 session 时沿用宿主的
活动会话惯例（与 `listTasks` 等全局端点同模式）。

### 4.6 skills-manager App（apps/skills-manager）

照抄 model-manager 骨架（build.mjs + tsconfig + manifest.json + src/）：

- **manifest.json**：`id: "skills-manager"`、title「技能管理」、icon；
  `ctx.slots.inject("nav.sidebar", ...)` + `ctx.slots.inject("surface.main", ...)`。
- **视图**（`skills-view.tsx`）：
  - 列表按 `source` 分组（project-onion / project-claude / project-dsh / project-agents /
    user-* / runtime…），组内按 rank；每行：名称、描述（截断）、来源徽标、
    model-invocable / user-invocable 标记、启停 Switch；
  - 搜索过滤（名称 + 描述）；
  - 顶栏显示禁用文件路径（`file` 字段）与禁用计数；
  - 开关切换 → 乐观更新 + `putSkillManager`（全量 disabled 列表），失败回滚 + toast；
  - 变更后重新拉 `GET /v1/skills` 刷新 enabled 状态。
- **无服务端数据以外的本地持久化**（启停状态全部在 skills-manager.json，UI 配置如需
  「确认开关」才走 localStorage，对齐 model-manager）。
- **安装**：`node build.mjs` → `dist/`，经 `/v1/apps/install` 或
  `skills/ava2-app-dev/scripts/install-app.ps1` 装入 AppHost。

## 5. 边界与非目标

- **不支持按来源禁用同名技能**（如只禁 claude 来源的 `foo`、保留 onion 来源）：
  名字即身份，保持简单；如后续需要，扩展 schema 为对象形式即可（向后兼容）。
- **不支持全局（跨工作区）禁用**：本期只有工作区文件；全局可后续加
  `<ONION_HOME>/skills-manager.json` 并做两层合并（同名任一命中即禁用）。
- **不监听文件变更推送**：不引入 watcher；目录消息本身在每步 pre-stage 重算，
  自然覆盖外部编辑；UI 侧手动刷新。
- **不动 frontmatter 语义**：`disable-model-invocation`（技能作者维度）与本次
  `disabled`（使用者维度）正交并存。

## 6. 实施步骤与验收

1. **内核**：`skill/disable.rs` + registry 钩子 + assemble 接线；
   单测：禁用后 list 不含、get 返回 None、文件非法 fail-open、无 policy 行为不变。
2. **REST**：`rest/skills_manager.rs` GET/PUT + `SkillInfo.enabled`；
   集成测试（`tests/rest_skills_manager.rs`）：PUT 后 GET 一致、非法名 400、
   原子写落盘、禁用后 `GET /v1/skills` enabled=false 且列表仍可见。
3. **宿主**：AgentChannel 三方法 + host 实现透传。
4. **App**：apps/skills-manager 全量（manifest + 视图 + 交互），
   `node build.mjs` 通过；手动验收：切换开关 → 新一轮对话目录消息更新
   （`/skills` 或日志确认 `<available_skills>` 替换）。
5. **端到端**：禁用某技能后在新会话提问该技能能力 → 模型不再提及；
   `/name` 手势退化为普通文本。

## 7. 开放问题

- App 端是否需要「临时启用（仅本会话）」覆盖？若需要，可在 REST 增加会话级
  override，内核 policy 增加 per-session 覆盖层——建议二期再做。
- 是否在 chatapp 的 composer 内提供 `/skills` 快捷入口跳转本 App（canvas.open 语义）？
  属 UI 增强，随 App 一期顺带评估。

## 8. 增量：技能广场（腾讯 SkillHub 接入，2026-11-22）

skills-manager App 增加第二页签「技能广场」，接入腾讯 SkillHub 社区市场
（skillhub.cloud.tencent.com），支持查询、查看、下载安装 skill 到本地。

### 8.1 上游契约（SkillHub CLI `skills_store_cli.py` 实测逆向确认，均匿名可用）

| 用途 | 端点 | 说明 |
| --- | --- | --- |
| 搜索 | `GET https://api.skillhub.cn/api/v1/search?q=&limit=` | `{results:[{slug,name,displayName,summary,version,downloads,stars,source,namespace,publisher,...}]}` |
| 详情 | `GET https://api.skillhub.cn/api/v1/skills/resolve?coordinate=@<namespace>/<slug>` | `{skill:{slug,displayName,summary,source,stats,sourceUrl,iconUrl,...}, latestVersion, owner}` |
| 下载（主链） | `GET https://api.skillhub.cn/api/v1/download?slug=<slug>` | 302 → CDN 版本化 zip（`/skills/<slug>/<version>.zip`） |
| 下载（兜底） | `GET https://skillhub-1388575217.cos.ap-guangzhou.myqcloud.com/skills/<slug>.zip` | COS 直链 |

- zip 内 `SKILL.md` 位于根目录，frontmatter `name` 为会话内调用名（可与 slug 不同）；
- **企业源技能（`source: "enterprise"`）详情可见但下载 404**，需企业登录才能下载，
  社区源（clawhub）可匿名完整安装。

### 8.2 架构：App 直连读面 + 服务端安装面（2026-11-22 修订）

**修订原因**：读面最初实现为服务端专用代理（`/v1/skill-hub/search` / `/skill`），
评审后改为 **App 直连上游**——范围收敛为开源（社区）技能，不做企业源特判。
浏览器 CORS 白名单（实测仅回显腾讯自家域 origin）使 App 无法直接 `fetch` 第三方
站点，故直连的落地形态是**宿主通用外联通道**：

| 层 | 职责 |
| --- | --- |
| `ctx.net.fetch`（`inject: ["net"]`） | App 侧唯一外联出口（统一联网服务能力面，见 `docs/design/net-service.md`）：`{ url, method?, headers?, body?, encoding? }` → `{ status, contentType, body }`；host 层按 `manifest.net.allow` 做 App 级域白名单（越权 fail-loud） |
| `POST /v1/net/fetch` / `GET /v1/net/fetch?url=`（`rest/net.rs`） | 受控外联转发：仅 https 公网（拒绝 localhost / 私网 / 环回 / 链路本地 / 无点主机名）、重定向逐跳复检、方法仅 GET/POST、请求头白名单、4 MiB 上限、超时 15s、上游状态码透传。平台级能力，任何 App 可用 |
| `POST /v1/skill-hub/install?session=<id>` | 安装面（必须在服务端）：slug 过 `is_skill_name` → 下载 zip（主链 302 → CDN 版本化 zip，兜底 COS 直链；仅社区源可匿名下载，企业源 / 未知 slug 404）→ `enclosed_name` 防 zip-slip 解包到 `<projectRoot>/.onion/skills/<slug>/` → SKILL.md 校验（缺 / 非法名 422 并清理半成品）→ `registry.refresh()`（`skills/change` 广播）→ `{slug, name, dir, files}`；已存在 409、非法 slug 400 |

- 安装目标 = `find_project_root(session_cwd)` 下的 `.onion/skills`（与技能发现
  provider 的 project-onion root 完全一致，安装后 filesystem provider 下次扫描即见）；
- 搜索 / 详情的上游契约（`api.skillhub.cn/api/v1/search` 与
  `/api/v1/skills/resolve?coordinate=@<ns>/<slug>`；namespace 兼容 handle 与
  canonical 全坐标 `@ns/slug`）由 App 侧（`skill-hub-view.tsx`）自行拼装解析；
- 服务端不再持有 SkillHub 搜索/详情端点与企业源 `installable` 特判。

### 8.3 宿主与 App

- `AgentChannel` 增 `installSkillHub(slug, sessionId)`（`src/host/agent.ts` +
  `types.ts`，SDK 类型随 `npm run build:sdk` 重建；契约清单
  `contracts/app-contract.json` 同步）；外联不在 AgentChannel，改为独立
  `ctx.net` 能力面（`src/host/registry.ts` 的域白名单 + `manifest.ts` 的
  `net.allow` 校验）；
- App `manifest.json` 声明 `inject: ["slots","agent","net"]` 与
  `net.allow: ["api.skillhub.cn","skillhub.cloud.tencent.com"]`；
- App `skills-view.tsx` 顶栏加「本地技能 / 技能广场」页签；广场视图
  （`skill-hub-view.tsx`）：搜索框 → 经 `ctx.net.fetch` 请求搜索 API 并自解析 →
  结果卡片（名称/摘要/版本/下载量/认证标记）→ 「详情」直连 resolve 元信息 →
  「安装」一键装到会话工作区（安装失败 409 / 404 原样报错；成功后自动刷新
  本地目录列表）。

### 8.4 测试

- `tests/rest_net.rs`（真实调上游）：外联通道取回 search JSON、上游 404 透传、
  SSRF 边界（私网 / 非 https / localhost / 无点主机名一律 400）；单元测试
  `net_tests.rs` 覆盖 `validate_public_https_url` 放行/拒绝清单；
- `tests/rest_skill_hub.rs`（真实调上游下载 API）：真实安装 → 落盘 + 注册表
  可见 → 重复安装 409、非法 slug 400、未知 slug 404 且不留半成品。
