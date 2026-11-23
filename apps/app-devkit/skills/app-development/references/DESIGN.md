---
title: ava2 App UI 设计指南
description: 面向 App 开发者的设计规范：令牌消费速查、排版阶梯、间距圆角纪律、交互反馈配方、App 常用组件观感、数据可视化锚点与反模式禁令。与宿主壳观感一致的全部数值内联于此，可直接照做。
keywords: ava2, App, UI 设计, design token, 交互规范, 组件配方
---

<!-- 本指南随 app-devkit 技能分发，面向 App 开发者编写（自包含、可移植）。
     数值均提取自 ava2 壳实测值；壳侧设计系统全文（含决策记录）见仓库
     docs/design/DESIGN.md，App 开发只需本文件。 -->

# ava2 App UI 设计指南

App 注入宿主舞台后与壳同屏——App UI 的观感必须与壳一致：**克制、0.5px 发丝线、pill 全圆、透明度反馈**。本文件给出 App 开发所需的全部设计数值与配方，照做即可与宿主同风格。

## 1. 设计气质

- **密度 6/10**：紧凑工作台风。控件高 24–44px，列表行信息密度高，无大面积留白装饰。
- **变化 2/10**：结构化布局（标题区 → 工具栏 → 内容区），对称、无装饰性构图。
- **动效 4/10**：轻快安静——0.12–0.2s 标准 ease，只动 fill / opacity / color 与 ≤6px 位移；无 spring、无编排动画。
- **强调色 ≤ 1**：唯一品牌蓝做主操作与选中态；红/绿/琥珀只做状态语义，永不装饰。

## 2. 令牌消费速查（App 一律 `var(--ava-*)`）

| 令牌 | 语义 | 浅色 | 深色 |
| --- | --- | --- | --- |
| `--ava-bg-base` | 页面画布 | `#ffffff` | `#151517` |
| `--ava-bg-layer-1` | 卡片 / 列表 / 输入底 | `#ffffff` | `#232324` |
| `--ava-bg-layer-2` | 浮层 / 弹窗底 | `#ffffff` | `#2c2c2e` |
| `--ava-bg-sidebar` | 次级分区底（比画布低半档） | `#f9fafb` | `#1b1b1c` |
| `--ava-label-primary` | 主文字 | `#0f1115` | `#f9fafb` |
| `--ava-label-secondary` | 次要文字、ghost 控件字 | `#61666b` | `#cfd3d6` |
| `--ava-label-tertiary` | 辅助文字、图标静息、占位 | `#81858c` | `#adb2b8` |
| `--ava-border-l1` | 分隔线（0.5px 发丝） | 黑 4% | 白 6% |
| `--ava-border-l2` | 控件边框 | 黑 10% | 白 12% |
| `--ava-border-l3` | 强分隔（侧栏缘等） | 黑 12% | 白 16% |
| `--ava-interactive-bg-hover` | hover 统一叠加 | `rgba(38,49,72,.06)` | `rgba(255,255,255,.08)` |
| `--ava-nav-active` | 选中实底 | `#ebeef2` | `#43454a` |
| `--ava-state-business-primary(-hover)` | 品牌蓝（主按钮 / 选中 / 徽标）+ hover 档 | `#4176e6` / `#5585ea` | `#679efe` / `#5b8ef0` |
| `--ava-state-success / danger / warning` | 状态语义 | `#12a150` / `#ec1313` / `#b26a00` | `#3ecf8e` / `#f25a5a` / `#f0b429` |
| `--ava-shadow-elevated / panel` | 轻 / 重浮起 | 2px10px6% / 8px24px8% | 2px14px45% / 8px28px50% |
| `--ava-space-0…5` | 间距六档 | 2 / 4 / 8 / 10 / 16 / 24px | 同 |
| `--ava-radius-xs…xl/panel/pill` | 圆角阶梯 | 4 / 10 / 12 / 16 / 20 / 28 / 999px | 同 |
| `--ava-font-ui / --ava-font-mono` | 字体栈 | 见 §3 | 同 |

规则：

- **禁止硬编码色值 / 字号**——一律令牌；主题由壳 `data-theme="light|dark"` 整体翻转，App 无需自己实现双套值。
- App 业务专有色（如气泡底）放 App 本地令牌（根类内定义 `--<app>-*`），仍亮暗各给一值。
- App **不得改写壳令牌值**、不得给壳根节点加样式。

## 3. 排版

双字体栈由壳统一下发（`--ava-font-ui` / `--ava-font-mono`），App 直接消费，**不自备字体栈**（自备即脱离主题；尤其禁前置 `system-ui` / `PingFang SC`——会拦截中文字形）。数字一律 `font-variant-numeric: tabular-nums`。

App 字号阶梯（root 14px 基准）：

| 字号/行高 | 字重 | App 用途 |
| --- | --- | --- |
| 11/18 | 400 | 极小标签下限：tag、角标 |
| 12/18 | 400 | 次要信息：时间戳、说明、页脚元信息 |
| 13/18–24 | 400–600 | **界面默认档**：按钮、tab、列表行、菜单项 |
| 14/22 | 400–500 | 正文段落、输入框 |
| 15/24 | 600 | 视图主标题（配合一句 12px 副标题） |
| 16/24 | 600 | 卡片主名 / 数值强调 |
| 18/28 | 700 | 页面大标题（字距 -0.2px） |

字重只用 **400 / 500 / 600 / 700** 四档。

## 4. 间距、控件高度与圆角

**间距六档**（`--ava-space-*`，语义固定）：0=2px 微缝（tab 间隙）· 1=4px 元素内最小距 · 2=8px 控件内边距 · 3=10px 图标↔文字内联 · 4=16px 页面外距 / 卡片间距 · 5=24px 分组留白 / 弹窗内边距。禁止新增档位或字面值大间距。

**App 常用控件高度**：24px（icon-btn / tag）· 26–28px（小按钮 / 列表行内控件）· 32px（列表行 / 菜单项 / 分段控件）· 34px（搜索框）· 38px（弹窗按钮）。

**圆角**：微控件（≤16px）用 `xs(4)`；按钮 / 输入框 / tab 用 `sm(10)`；卡片 / 列表容器用 `md(12)–lg(16)`；大图标容器 `xl(20)`；**按钮 / 搜索框 / tag / 开关 / chips 一律 `pill(999)` 全圆**——ava2 高频形态。

**边线**：分离一律 **0.5px 发丝线**（分隔 l1、控件边框 l2），禁 ≥1px 实线。

## 5. 交互纪律

- **状态三板斧**：hover = 交互叠加（wash）+ 文字提亮一档 → active/选中 = `--ava-nav-active` 实底（+ 字重 500）→ disabled = opacity 0.45。**色相永不漂移**——反馈靠透明度与档位，不换基色。
- **过渡**：120–200ms 标准 ease；浮层显隐用 opacity（布局零位移）。
- **焦点**：键盘导航走原生 `:focus-visible`（2px 品牌蓝 outline + 2px offset），禁改 `:focus` 全量显示。
- **实底按钮 hover**：用品牌蓝专用 hover 档（`--ava-state-business-primary-hover`），非 wash。
- **危险操作**：常态弱化为次级外观，hover 才转红——红只在不可逆操作瞬间出现。

## 6. App 组件配方（观感对齐壳）

| 组件 | 配方 |
| --- | --- |
| **分段控件（tabs）** | 32px 高；容器 `--ava-bg-sidebar` 底 + radius-sm + 0.5px l2 边 + 2px 内衬；选中项 `--ava-button-elevated-fill` 底 + radius-sm(8px) + 阴影 elevated + 字重 500；未选中 secondary，hover wash |
| **主按钮** | 38px pill、品牌蓝实底白字 13px/500，hover 切 hover 档 |
| **次按钮** | 0.5px l2 描边 pill、透明底，hover wash |
| **危险按钮** | 描边 pill，常态 secondary 字色，hover 才转 danger 字 + danger 8% 底 |
| **输入框** | 0.5px l2 描边、radius-sm 或 pill、`bg-layer-1` 底；focus 描边转品牌蓝（无 ring）；placeholder tertiary |
| **tag / 徽标** | pill、11px、18px 行高、0.5px l2 边；语义态换状态色字 + 边框 |
| **列表行** | 32–38px、radius-md（容器 radius-lg + 0.5px l1 边 + `bg-layer-1` 底）；行间 0.5px l1 分隔（末行无边）；hover wash；选中 nav-active 实底 |
| **卡片** | `bg-layer-1` 底、radius-lg、0.5px l2 边（浮出层再加 shadow-elevated） |
| **开关** | 40×24 pill、白 knob 18px、on = success 绿实底、0.2s 滑动 |
| **骨架屏** | `--ava-bg-sidebar` 底圆角块，opacity 0.45↔0.9 脉动 1.2s；**禁 spinner** |
| **空态 / 错误态** | 居中图标位（44px radius-md wash 底）+ 13px 说明 + 行动按钮；不留白屏 |
| **滚动条** | 5px 细条、透明轨道、pill thumb（`--ava-scrollbar-thumb`，hover 加深） |
| **浮层 / 弹窗** | `bg-layer-2`、radius-lg、0.5px l2 边 + shadow-panel、入场 scale(0.96)+translateY(6px) 0.18s；遮罩 `rgba(0,0,0,.35)` |

## 7. 数据可视化锚点（榜单 / 统计类必备）

纯文本行平铺是评审即拒的形态。数据列表必须有视觉锚点：

- **排名徽标**：固定宽（22–28px）居中数字；前三名品牌色 + 加粗，其余 tertiary；可配名次档位底色（金/银/铜用 wash 浓度区分，不引入新色相）。
- **量值可视化**：mini bar（品牌蓝，宽度按对数归一）、数值徽章或中文量级（万 / 亿）；tabular-nums 右对齐、固定宽度防跳动。
- **语义标签**：新=success、热=danger/warn、沸=品牌蓝 pill；禁止全灰一色。
- **视觉焦点**：每屏有一个主内容焦点（大数字 / 榜首卡片 / 图表），其余降级为次要层次。
- **外链行**：整行可点 + hover 明确反馈；收藏星 / 外链箭头等点缀用 24px 网格单色 SVG（见 `references/icon-spec.md`）。

## 8. 布局结构（三段式）

```
┌ 标题区：主标题 15–21px/600 ＋ 一句 12px 副标题（说明用途/数据源）
├ 工具栏：分段控件（左）· spacer · 状态信息 12px（右）· pill 操作按钮
├ 内容区：flex:1 + overflow-y:auto（列表 / 卡片 / 图表）
└ 页脚：11–12px tertiary 元信息（更新时间、数据来源、计数）
```

区块间距走令牌节奏（16px 分区 / 8px 组件间）；**禁止均匀堆砌**（所有元素同字号同灰度一列到底）。

## 9. 反模式禁令（评审即拒）

1. 禁硬编码色值 / 字号——一律令牌。
2. 禁多于一个强调色——品牌蓝之外红绿琥珀只做状态语义。
3. 禁色相漂移反馈——hover/active 用透明度叠加或档位实底。
4. 禁 ≥1px 实线边框——一律 0.5px 发丝线。
5. 禁纯黑（深色底 `#151517`；`#000` 只出现在遮罩 rgba）。
6. 禁字体栈自备或前置 system-ui / PingFang SC。
7. 禁中间字重（只用 400/500/600/700）。
8. 禁转圈 spinner——加载用骨架屏脉动。
9. 禁单主题交付——亮暗两态都必须验证。
10. 禁均匀堆砌无视觉焦点、列表无排名/无徽章/无分组、无 hover 反馈。
11. 禁改写壳令牌 / 污染壳样式——样式限域在 App 根类下。
12. 禁自备字体栈与 mono 栈删中文等宽兜底。

## 10. 交付核对

- [ ] 亮 / 暗主题截图各一张，观感一致
- [ ] 全部颜色 / 字号 / 间距 / 圆角可溯源到令牌
- [ ] 交互三态（hover / active / disabled）齐备，过渡 120–200ms
- [ ] 数据列表有排名 / 量值 / 语义标签锚点，tabular-nums
- [ ] 空态 / 错误态 / 加载态（骨架屏）三态齐备
