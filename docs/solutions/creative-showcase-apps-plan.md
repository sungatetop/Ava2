---
title: 创意示例 App 矩阵方案（跨领域推广）
description: 为推广 ava2 而设计的五个跨领域创意 App 方案，含受众定位、核心体验、slot/ctx 契约映射、Agent 用法、推广素材点与原型索引，供后续开发直接落地。
keywords: 创意 App, 示例应用, 推广, AppHost, slot, AgentChannel, 原型, showcase
author: AI 助手
date: 2026-11-11
status: review
category: solutions
---

# 创意示例 App 矩阵方案（跨领域推广）

## 1. 目标与推广逻辑

推广 ava2 的难点不是「功能列表」，而是让人**在 30 秒内看懂它和普通 AI 客户端的差别**。差别只有三条，所有示例 App 都必须把这三条演出来：

| 能力 | 观众看到的样子 |
| --- | --- |
| **一切皆 App**（slot 装配） | 同一个工作台里，导航、主区、右侧画布、下面板同时被不同 App 占据，且彼此不打架 |
| **Agent 是通道不是页面** | App 自己设计交互，Agent 只是被调用的能力（闯进 App 的界面里干活，而不是把用户赶去聊天框） |
| **多舞台并行** | 主区做「操作」，画布做「全局视图」，下面板做「追问」，三块同时活着 |

因此选型不追求「功能最多」，追求**每个领域一个能被截图传播的场景**。

## 2. 选型原则

1. **一个 App 一个领域、一个主张**：每个示例只讲一件事，避免功能堆砌。
2. **只用公开契约**：`slots` / `events` / `agent` / `storage` 四面，不碰宿主特权，保证示例即最佳实践。
3. **零后端**：数据来自工作区文件或用户粘贴，演示环境不需要额外服务。
4. **视觉差异化但同源**：统一消费 `--ava-*` 令牌，仅用 App 本地强调色区分气质（见 §4）。
5. **可拆可合**：单个 App 独立可演示；五个一起装则构成一个完整的「跨领域工作台」故事。

## 3. 六个示例 App

> 第六个（总管家 Butler）是 ava-app 已有设计的 App 化移植：其产品与交互设计已在 `ava/docs/design/ava-super-agent-butler-design.md`（产品）与 `ava/docs/design/ava-butler-architecture-overview.md`（架构）确认落地，本方案只做 **ava2 App 契约映射**，不重复发明。

### 3.1 故事织机 · Storyloom（内容创作 / 网文与编剧）

- **一句话**：把「灵感 → 节拍 → 成稿」放在一块屏幕上，Agent 只在你指定的节拍里动手。
- **受众**：网文作者、短剧编剧、自媒体脚本作者。
- **痛点**：AI 写作工具把稿子抢走了——整段重写、风格漂移、结构失控。
- **核心体验**：
  - 主区左栏是三幕节拍板（可拖动排序、每拍有情绪值与字数预算），右栏是稿纸；选中某一拍，Agent 只在这拍的上下文里续写，**越界即拒**。
  - 画布是**角色关系图**：人物节点带出场章节热度，点节点即筛选该角色所有场景。
  - 下面板是「节奏诊断」：把当前章节的张力曲线与同类型作品基线比对，给出可点击的修改建议。
- **契约映射**：
  - `surface.main`（认领 `storyloom`）+ `nav.sidebar`
  - `surface.canvas`（认领 `story-map`）+ `panel.canvas`（follow `story-map`）
  - `panel.sidebar`：章节字数预算 / 今日目标
  - `ctx.agent`：`send` / `onSessionEvent` / `getTodos`（把节拍落成 todo）/ `getPlan`
  - `ctx.storage`：节拍表、角色卡、风格样本
- **推广素材点**：左「节拍板」右「稿纸」的二分屏截图 + 一句「AI 不许改你的结构」。
- **复杂度**：中（无构建依赖也可做，图表用 SVG）。

### 3.2 苏格拉底实验室 · Socratic（教育 / 教师与学生）

- **一句话**：不给你答案，只给你下一个问题；每轮追问都落进「掌握度图谱」。
- **受众**：中学/大学理科教师、备考学生、教研组。
- **痛点**：AI 直接给答案，学生抄完就忘；教师看不到「哪一步卡住」。
- **核心体验**：
  - 主区是**追问式对话**：Agent 被约束为「只提问 + 只在学生连续两次卡住时给最小提示」，每题结束自动提炼知识点。
  - 画布是**解题板书**：分步推导卡片，每步标注「学生自述 / 提示 / 订正」来源，可直接投屏。
  - 下面板是**掌握度热力条**：知识点 × 掌握等级，红色格子点击即生成同类变式题。
  - `user.menu` 提供「一键导出课堂小结」。
- **契约映射**：
  - `surface.main` + `nav.sidebar`
  - `surface.canvas`（`socratic-board`）+ `panel.canvas`
  - `panel.sidebar`：本节课知识点清单与耗时
  - `ctx.agent`：`createSession` / `send` / `interrupt` / `answerQuestion`（Agent 反问时由学生作答）/ `getTodos`
  - `ctx.storage`：掌握度矩阵、错题本
- **推广素材点**：板书卡片三种来源色标 + 「AI 忍住不给答案」的反差文案。
- **复杂度**：中（核心是提示词约束 + 状态机，UI 简单）。

### 3.3 数据叙事台 · Insight（运营 / 市场分析）

- **一句话**：把一份 CSV 变成一份能被复述的结论，而不是一排没人看的图。
- **受众**：运营、市场、电商店主、小微企业主（不会 SQL 的人群）。
- **痛点**：BI 工具门槛高；聊天式分析给不出可复用结论，来回追问成本高。
- **核心体验**：
  - 主区上部是数据集卡（从工作区拖入 / 粘贴），下部是**洞察卡流**：每张卡一句话结论 + 支撑数字 + 「为什么」折叠说明 + 置信度。
  - 画布是**图表墙**：结论驱动的 3 张主图（趋势 / 结构 / 异常），点图表范围即在主区追问。
  - 下面板是**口径与假设**：本次分析用到的字段、时间窗、排除项，一键写回备注（可审计）。
  - 侧栏面板展示数据健康度（缺失率、异常值）。
- **契约映射**：
  - `surface.main` + `nav.sidebar`
  - `surface.canvas`（`insight-charts`）+ `panel.canvas`
  - `panel.sidebar`：数据体检
  - `ctx.agent.listDirs` / `readWorkspaceFile`（读工作区 csv）+ `send` + `onSessionEvent`
  - `ctx.storage`：字段口径字典、历史结论
  - `ctx.events.on("canvas.open")`：结论卡「在画布展开」联动
- **推广素材点**：「一句话结论 + 三张图 + 口径可审计」的三段式截图。
- **复杂度**：偏高（解析 CSV + 图表；图表可用纯 SVG 折线/柱）。

### 3.4 代码考古 · RepoAtlas（开发者 / 工程团队）

- **一句话**：接手陌生仓库时，先看地图，再动手改。
- **受众**：新入职工程师、外包接手方、开源贡献者、技术负责人。
- **痛点**：AI 改代码很勇敢，但没人知道改动会波及哪里；文档早就过期。
- **核心体验**：
  - 主区是**改动影响面**：输入「我想改 X」，列出直接依赖、间接影响、需要补的测试与风险等级。
  - 画布是**架构地图**：模块节点按目录聚类，边是调用/依赖，点节点看「谁在用它」。
  - 下面板是**考古笔记**：每个模块一句话职责 + 最近变更痕迹 + 可疑点（TODO/超长函数），Agent 生成后人工可编辑。
  - `user.menu`：「生成接手清单」（落成 todo）。
- **契约映射**：
  - `surface.main` + `nav.sidebar`
  - `surface.canvas`（`repo-atlas`）+ `panel.canvas`
  - `panel.sidebar`：仓库体检（体积、模块数、TODO 数）
  - `ctx.agent`：`listDirs` / `readWorkspaceFile` / `send` / `getTodos` / `setPlan`（把改动拆成 plan）/ `getPlan`
  - `ctx.storage`：模块笔记、风险白名单
- **推广素材点**：架构地图 + 影响面清单的同屏截图，配「AI 动手前先画地图」。
- **复杂度**：偏高（地图布局算法；演示版可用固定布局）。

### 3.5 晨间简报 · Morningside（通用知识工作者 / 生活）

- **一句话**：打开工作台的第一屏，就是今天该做什么和为什么。
- **受众**：所有知识工作者（也是最广的「第一印象」示例）。
- **痛点**：AI 客户端空着一个输入框等你提问——冷启动成本高。
- **核心体验**：
  - 主区是**简报卡流**：昨日进展摘要、今日三件要事（来自 todo/goal）、待回填审批、需要你决策的一个问题。
  - 侧栏面板是**目标进度环**（`getGoal`）与今日专注时长。
  - 画布是**日程时间轴**（今天的时间块与冲突提醒）。
  - `user.menu`：快捷记录（一句话丢进收件箱，Agent 稍后归类）。
- **契约映射**：
  - `surface.main` + `nav.sidebar` + `user.menu`
  - `surface.canvas`（`morning-timeline`）+ `panel.sidebar`
  - `ctx.agent`：`listSessions` / `getTodos` / `getGoal` / `getPending` / `replyApproval` / `answerQuestion`
  - `ctx.storage`：简报偏好、收件箱
- **推广素材点**：空状态对比图（左边普通聊天框空转 / 右边简报已就绪）。
- **复杂度**：低（纯读接口 + 卡片排版），**最适合作为首个落地示例**。

### 3.6 总管家 · Butler（通用 · 系统级助理）

> 源自 ava-app 已有设计（`ava/docs/design/ava-super-agent-butler-design.md` §5.9 / `ava-butler-architecture-overview.md`），本条目给出其 App 化映射。

- **一句话**：工作台的常驻智能体——跨会话/跨工作空间替你派发任务、盯结果、把关审批，一切汇报都在一个微信式对话窗里完成。
- **受众**：所有知识工作者；这是「Agent 是系统级能力，而不是一个聊天页」的最佳注脚。
- **核心体验**（对齐 ava-app 设计）：
  - **主区 = 微信式对话流**（认领 `butler`）：与总管家像与人一样对话；butler 的一切主动回应都呈现在这里——答复 / 汇报 / 晨报 / 升级通知卡片。
  - **一句话委派**：用户在输入框说「整理周报 + 修 deprecation 警告」，butler 创建 worker 会话派发，以**动作条**内联回复「已创建会话 · 整理周报 · sid=a1b2 → 打开」。
  - **L4 升级通知卡片**：高危审批或委派失败时，对话流出现强调卡——「会话 B 请求 cargo publish，风险…建议…」+「转到会话」按钮（跳转该 worker 会话用既有审批 UI 处理，butler 不代理交互）。
  - **画布 = 派发看板**（认领 `butler-roster`）：roster 投影——每个派发会话一行（任务一句话 + 状态点 + workspace + 耗时），可点击跳转。
  - **下面板 = 信箱事件流水**（只读）：会话开始/结束、审批请求（「已按安全纪律代批 · 条款 R2」/「已通知用户到会话 B 处理」）、晨报/巡检简报；未读徽标计数。
  - **panel.sidebar = 注意力设置**：kill_switch（暂停自治）/ 静默时段 / 纪律档位（仅升级 / 纪律代批 / 宽松代批）。
- **ava-app 能力 → ava2 契约映射**（关键差异：ava-app 用五件工具 + 专属 WS 事件；ava2 App 一律走 ctx 通道 + 自定义总线事件）：

| ava-app 能力 | ava2 App 映射 |
| --- | --- |
| 悬浮球 / 独立 WebviewWindow | `user.menu` 入口 + `surface.main` 对话面板（App 内自管窗口形态） |
| 表达与执行分离的对话日志 | `ctx.storage` 持久化对话流 + 展示层过滤（只渲染双方发言） |
| create_session 派发 | `ctx.agent.createSession` / `ctx.agent.send` + `onSessionEvent` 订阅结果 |
| send_message_to_user 汇报 | App 内对话流渲染 + `ctx.events.emit`（`agent.reply` 定向回执） |
| resolve_approval 代批 | `ctx.agent.getPending` + `replyApproval`；**升级走对话流强调卡 +「转到会话」**，不阻塞 |
| butler_notice L1-L4 注意力分级 | 自定义总线事件 `butler.notice`（事件名加 App id 前缀）+ `ctx.storage` 未读计数 |
| roster 台账投影 | `ctx.agent.listSessions` + `onSessionEvent` 实时状态 |
| 晨报（降级项累积） | `ctx.agent.getTodos` / `getGoal` 聚合 + 主区晨报卡 |

- **推广素材点**：一张图同时出现「对话流动作条 + L4 升级卡 + 派发看板」——把「常驻智能体替你干活」讲清楚。
- **复杂度**：中高（会话状态订阅 + 升级卡片交互），但复用 ava-app 已验证的交互，风险低。
- **验收基线**：对话流不渲染 agent loop 中间消息；L4 卡点击跳转对应会话；信箱只读、无操作按钮。

## 4. 视觉语言

统一骨架与令牌，靠 App 本地强调色与信息密度区分气质：

| App | 本地强调色（App 内令牌） | 排版气质 | 密度 |
| --- | --- | --- | --- |
| Storyloom | 暗金 `#b8873d` | 衬线标题 + 宽行高稿纸 | 中 |
| Socratic | 青绿 `#1f9e8f` | 板书感等宽数字 + 卡片描边 | 中高 |
| Insight | 靛蓝 `#3f5bd9` | 数字大字 + 表格化对齐 | 高 |
| RepoAtlas | 石墨橙 `#c2632a` | 等宽标签 + 节点图 | 高 |
| Morningside | 中性墨 + 蓝强调 | 大留白卡片流 | 低 |
| Butler | 墨绿 `#2e7d63` | 微信式气泡 + 状态点行 | 中高 |

硬性约束（与 [DESIGN.md](../design/DESIGN.md) 一致）：底色/文字/边框/圆角/间距一律消费 `--ava-*`；强调色只用于该 App 的业务焦点，禁止大面积铺色；亮暗两套主题都要可读。

## 5. 原型索引

| App | 原型 | 演示重点 |
| --- | --- | --- |
| 总览 | [showcase-index.html](../design/prototypes/showcase/showcase-index.html) | 五个 App 的宫格与领域标签 |
| Storyloom | [storyloom.html](../design/prototypes/showcase/storyloom.html) | 节拍板 × 稿纸二分屏 + 角色关系画布 |
| Socratic | [socratic.html](../design/prototypes/showcase/socratic.html) | 追问对话 + 解题板书 + 掌握度热力 |
| Insight | [insight.html](../design/prototypes/showcase/insight.html) | 洞察卡流 + 图表墙 + 口径面板 |
| RepoAtlas | [repoatlas.html](../design/prototypes/showcase/repoatlas.html) | 影响面清单 + 架构地图 |
| Morningside | [morningside.html](../design/prototypes/showcase/morningside.html) | 简报卡流 + 目标环 + 时间轴 |
| Butler | [butler.html](../design/prototypes/showcase/butler.html) | 对话流动作条 + L4 升级卡 + 派发看板 + 信箱 |

## 6. 落地路径

1. **先做 Morningside**（1 个 App、纯读接口、无构建依赖）：验证「第一屏就有内容」的推广叙事，产出截图与 30 秒演示视频。
2. **再做 Butler**（ava-app 已有设计、交互已验证）：把「常驻智能体」的叙事立起来，是推广价值最高的一个；先映射对话流 + 派发看板，再补 L4 升级卡与信箱。
3. **再做 Storyloom 与 Socratic**：两者共享「主区操作 + 画布全局视图 + 下面板追问」结构，可复用样式与布局组件。
4. **Insight 与 RepoAtlas** 最后做：需要工作区文件读取与图表/图布局，作为「专业场景」的深度示例。
5. 每个 App 按 [ava2-app-dev 技能](../../skills/ava2-app-dev/SKILL.md) 与 [apps-monorepo-and-dual-target-evaluation.md](../design/apps-monorepo-and-dual-target-evaluation.md) 的**目录级独立 TS 单元形态**交付：`apps/<id>/` 独立 package.json + tsconfig（`@ava-ai/app-sdk` 包依赖，d.ts 由 `npm run build:sdk` 从宿主 `src/host/types.ts` 投影）+ esbuild 构建 → `dist/`（react 系 external，宿主 import map 供给），`npm run check`（tsc --noEmit）通过后 `validate-manifest.mjs` 自检 `dist/`，再 REST 安装 `dist/`，bump version 迭代。
6. 统一宣传口径：**「同一个工作台，六种专业界面；Agent 在界面里干活，管家在后台替你盯着。」**

## 7. 验收清单（每个示例 App）

- [ ] `node skills/ava2-app-dev/scripts/validate-manifest.mjs apps/<id>` 0 error。
- [ ] 安装后导航项、主区、画布、下面板同时出现且互不冲突；`viewId` 全局唯一。
- [ ] Agent 调用有失败兜底（不可达时显示明确文案，不白屏）。
- [ ] 卸载/禁用后 slot 干净撤回，无残留 tab。
- [ ] 亮/暗主题都可读，无硬编码色值（App 本地令牌除外）。
- [ ] 冷启动 3 秒内出首屏骨架（不阻塞在 Agent 请求上）。

**Butler 专属验收**：

- [ ] 对话流只渲染双方发言，不显示 agent loop 中间消息（对齐「表达与执行分离」）。
- [ ] L4 升级卡点击「转到会话」跳转对应 worker 会话；butler 自身不阻塞等待。
- [ ] 信箱只读展示处理状态（「已代批 · 条款 R2」/「已通知用户」），无审批操作按钮。
- [ ] 注意力设置（kill_switch / 静默时段 / 纪律档位）经 `ctx.storage` 持久化，重开生效。
