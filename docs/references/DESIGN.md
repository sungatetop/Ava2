---
title: DESIGN.md — DeepSeek Harness Web UI 设计系统
description: 提炼 dsh（deepseek-harness）web ui 的设计 token、字体、间隔、圆角、阴影与交互体系，形成 agent 友好的设计规范，作为生成或改造 UI 时的唯一设计基准。
keywords: dsh, deepseek harness, design token, 设计系统, 字体阶梯, 交互规范, DESIGN.md
author: AI 助手
date: 2026-11-11
status: done
category: references
---

# Design System: DeepSeek Harness Web UI（dsh）

> 形态参考 [google-labs-code/design.md](https://github.com/google-labs-code/design.md)：面向 AI 代理的
> 单一设计事实源——语义命名 + 精确值 + 功能角色 + 显式禁令。所有数值提取自 dsh 源码
> `packages/client/ui-theme/src/styles/`（token 权威）与 `ui-primitives` / `ui-chat` 组件实测，
> 非估算值。

## 1. Visual Theme & Atmosphere

**「克制的工具型桌面应用」**——冷静、安静、信息优先。蓝灰中性基底 + 单一品牌蓝，
一切交互反馈通过**半透明填充与透明度**表达而非色相变化；边界用 **0.5px 发丝线**而非实线；
浮层是**半透明玻璃**（blur 40px）；阴影极淡到几乎不存在，深色模式完全靠描边承担层次分离。

- **Density：6/10**（每日工具偏紧凑——28–44px 控件高度，行级信息密度高，但留白从容）
- **Variance：2/10**（严格对称的结构化布局，无装饰性构图）
- **Motion：4/10**（轻快安静：80–300ms、标准 ease，只动 fill/opacity/color，无弹跳、无位移编排）

氛围参照：Linear / Arc 的克制传统，而非 Dribbble 式表演性界面。

## 2. Color Palette & Roles

双层体系：`--dsw-static-*` 原色层（Figma 导出，两主题下大多同值）→ `--dsw-alias-*` 语义层
（组件唯一消费面，`body[data-ds-dark-theme]` 整体翻转）。**组件只允许引用 alias，不得直接用 static。**

### 2.1 表面与文字（浅色 → 深色）

- **Canvas Base** `#FFFFFF` → `rgb(21,21,23)`（neutral-bluish-950）— 全局画布 `bg-base`
- **Sidebar Fill** `rgb(249,250,251)` → `rgb(27,27,28)` — 左栏专用底色（比画布低半档）
- **Layer 1/2/3** `#FFFFFF` → `rgb(35,35,36)` / `rgb(44,44,46)` / `rgb(53,54,56)` — 浮层梯度
- **Ink Primary** `rgb(15,17,21)` → `rgb(249,250,251)` — 主文字（**禁用 `#000000` 做深底或正文黑**，浅色文字黑是 bluish-1000 而非纯黑）
- **Ink Secondary** `rgb(97,102,107)` → `rgb(207,211,214)` — 次要文字、描述
- **Ink Tertiary** `rgb(129,133,140)` → `rgb(173,178,184)` — 辅助文字、图标静息色
- **Ink Dimmed** `rgb(225,229,238)` → `rgb(67,69,74)` — 占位/禁用文字
- **Hairline L1–L4** `rgba(0,0,0,0.04/0.10/0.12/0.16)` → `rgba(255,255,255,0.06/0.12/0.16/0.20)` — 0.5px 发丝边框四档透明度阶梯
- **Skeleton** `rgba(0,0,0,0.04)` → `rgba(255,255,255,0.08)` — 骨架屏填充

### 2.2 唯一品牌强调色（Accent ≤ 1）

- **DeepSeek Blue** `rgb(65,118,230)`（浅）/ `rgb(122,170,255)`（深）— 品牌蓝：主链接、
  info 按钮、focus ring、选中态、业务主状态（`state-business-primary`）。深色模式用提亮一档的 400
  保证暗面可读性；hover 方向相反（浅色 500→400，深色 400→500）。

### 2.3 状态色（只做语义，不做装饰）

- **Success** `rgb(34,197,94)`（secondary `rgb(78,209,126)`；底 `rgb(230,250,237)` / 深底 `rgb(35,60,44)`）
- **Error** `rgb(236,19,19)`（浅）/ `rgb(242,90,90)`（深）；底 `rgb(254,226,226)`
- **Warn** `rgb(245,158,11)`（label `rgb(221,134,41)`；底 `rgb(254,245,231)`）
- **Idle** `rgb(212,212,212)` / 深色 `rgb(127,130,135)` — 未激活/离线

### 2.4 交互填充（核心机制：透明度叠加，不改色相）

- **Hover Wash** `rgba(38,49,72,0.06)` → `rgba(255,255,255,0.08)` — 所有可 hover 表面的统一叠加
- **Active Wash** `rgba(38,49,72,0.10)` → `rgba(255,255,255,0.14)` — 按压态
- **Hover Accent Wash** `rgba(38,49,72,0.14)` → `rgba(255,255,255,0.24)` — 强调版 hover
- **Toast Invert** 底 `rgb(41,41,41)` / `rgb(67,69,74)` + 字 `#FFFFFF` — Toast 一律深底浅字（倒置层次）
- **Menu Glass** `rgba(248,249,250,0.58)` / `rgba(67,69,74,0.45)` + `backdrop-filter: blur(40px) saturate(150%)` — 菜单/浮层玻璃面；遮罩另用 `rgba(0,0,0,0.24/0.48)` 三档

## 3. Typography Rules

三字体栈，全平台 CJK 优先（**code 栈禁止裸 `monospace` 尾坠**——Windows CJK 会回退宋体）：

- **UI Sans**（正文/界面）：`-apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', 'Helvetica Neue', sans-serif`
- **Brand Display**（品牌/引导页仅限）：`'Montserrat'`（本地 woff2，仅 300/400/500 三档）
- **Code Mono**（代码/工具名/token 数值）：`'SF Mono', 'JetBrains Mono', 'Fira Code', Consolas, 'Liberation Mono', Menlo, Courier, 'PingFang SC', 'Microsoft YaHei'`

### 3.1 字号阶梯（size/line-height 成对，px）

| 令牌 | 字号/行高 | 字重 | 用途 |
| --- | --- | --- | --- |
| `font-xl-24` | 24/32 | 600 | 页面大标题（罕用） |
| `font-l-20` | 20/28 | 500 | 区块标题 |
| `font-m-18` | 16/28 | 500 | 弹窗标题 |
| `font-base-16` | 16/24 | 400 | 正文强调档 |
| `font-s-14` | 14/22 | 400 | **界面默认档** |
| `font-xs-13` | 13/20 | 400 | 次要信息 |
| `font-xxs-12` | 12/18 | 400 | 标注/时间戳/pill |
| `font-xxxs-11` | 11/14 | 400 | 极小标签（下限） |
| `markdown-h1/h2/h3` | 21/30、19/28、18/26 | 700 | markdown 标题 |
| `markdown-h4` | 14/24 | 600 | markdown 四级（=正文档+字重） |
| `markdown-code` | 12/19 | 400 | 行内代码 |
| `markdown-code-block` | 11/19 | 400 | 代码块 |

### 3.2 字重纪律

只允许 **400 / 500 / 600 / 700** 四档。Figma 的 510（SF Pro 可变字重）落地一律 `500`——
非可变 webfont 的中间字重在跨平台渲染不可预测。

### 3.3 内容字号缩放机制（dsh 签名特性）

用户可在设置中调内容字号（10–22px，默认 14）。正文档与次要档随 `--dsh-content-font-size` 的
**像素差值**（delta）整体平移，markdown 阶梯按 0.875 等比缩放取整；secondary 档 = 尺寸 −1 且
**上限锁 13px**；small/code 固定不变。任何跟文字联动的图标按同一 delta 缩放。

## 4. Spacing, Radius & Shape

### 4.1 间距：4px 网格字面值（无 spacing token）

dsh **不设间距令牌**，组件直书 4px 基准网格字面值，档位经约定收敛：
`4 / 6 / 8 / 10 / 12 / 14 / 16 / 20 / 24`。横向 padding 惯例：紧凑控件 8–10、标准控件 14、
容器内边距 12–16、分区 20–24。

### 4.2 控件高度阶梯

| 高度 | 用途 |
| --- | --- |
| 24px | pill / tag / 状态徽标 |
| 28px | 小按钮（sm）/ 图标操作钮 / 消息操作行 |
| 32px | 输入框 / ghost 按钮 |
| 36px | 标准按钮（md） |
| 38–44px | 列表行 / 轮次头（内容密集区） |

### 4.3 圆角与形状（dsh 签名特性）

`xs 4 / sm 8 / md 12 / lg 16 / xl 20 / panel 28`，控件取 md 及以下，浮层取 lg–xl，大面板 28。
两条硬规则：

1. **全局平滑圆角**：支持 `corner-shape` 的引擎下，一切圆角沿 `superellipse(1.5)` 曲线
   （介于正圆与 squircle 之间），赋予控件「果冻感」的精致轮廓。
2. **全圆形状显式退出**：pill（999px）、圆点（50%）等不受限圆角必须配对
   `corner-shape: round`，否则 superellipse 会把圆压成方角胶囊。

## 5. Elevation & Shadow

- **Legacy 阴影三档**：`lv1 0 2px 4px rgba(0,0,0,0.05)`、`lv2` 双层（≤0.04）、
  `lv3 0 12px 32px rgba(0,0,0,0.08)`——全部极淡。
- **Elevation 三件套**（浮层/面板/面型按钮）：`0.5px 发丝描边画在 box-shadow 内`
  （组件配 `border: 0` 不占布局）+ 两层柔光（透明度 0.02–0.05）。
  - `panel`：描边 + `0 3px 8px 0.03` + `0 0 16px 0.02`
  - `prominent`：描边 + `0 3px 8px 0.04` + `0 0 20px 0.05`
  - `soft`（输入框）：描边 + `0 4px 16px 0.03` + `0 0 24px 0.03`
- **深色模式投影失效是预期**：柔光几乎不可见，**层次分离完全由描边承担**。
- 浮层（菜单/弹层）走玻璃面：半透明 fill + `blur(40px) saturate(150%)`，而非加浓阴影。

## 6. Component Stylings

- **Buttons**：36/28 两档高，radius md/sm；primary = 黑底白字（浅色模式主按钮是中性黑而非品牌蓝！品牌蓝只做 info 类），hover 换 `button-primary-hover`；ghost/outline hover 一律 Hover Wash 叠加；active 再进 Active Wash；disabled `opacity 0.4`。**无外发光、无渐变、无位移按压**。
- **Pill/Tag**：24px 高、999 全圆（配 corner-shape round）、12px 字、gap 4、内边距 8；hover 半透明；active 用 `inset 0 0 0 1px` 内描边环，不加投影。
- **Input**：32px 高、0.5px `border-l4` 描边、radius md、bg-layer-1；focus-within 描边变品牌蓝（**不是 focus ring**）；placeholder 用 dimmed 档。
- **Icon Action**：28px 方块、radius sm、静息 tertiary 色；hover 半透明填充 + 提亮文字；不可用 `opacity 0.4` 且保留 hover 以承载 tooltip 解释。
- **MenuSurface**：radius lg；玻璃材质单独 `position:absolute; z-index:-1` 的 material 层（只滤背景不动布局）；macOS 因原生 vibrancy 需近不透明兜底。
- **Toast**：深底浅字倒置（`toast-bg` neutral-800/750 + `toast-label` 白）；品牌蓝提亮一档才够暗面可读。
- **Scrollbar**：全局 5px thin、透明轨道、pill thumb 走 `--dsh-scrollbar-*` token；浮层经重绑 token 换 l2 档色，不写死。
- **Skeleton**：`bg-skeleton` 半透明填充 + shimmer 渐变扫过，匹配布局尺寸，**禁用转圈 spinner**。

## 7. Motion & Interaction

- **节奏令牌**：`fast 0.1s / standard 0.2s / slow 0.3s`，一律 `cubic-bezier(0.4, 0, 0.2, 1)`。
  显隐过渡可用 `80ms ease` 微档。**无 spring、无编排、无位移/缩放动画**——动效只动 fill/opacity/color。
- **Hover 显隐（data-actions-reveal）**：历史轮次操作行静息 `opacity: 0`，hover / focus-within 时
  `opacity: 1`（80ms）。用 opacity **而非 display**，布局零位移；整包 `@media (hover: hover)`，
  触屏/无 hover 设备全部常显。
- **Focus Ring 纪律**：宽 2px、色品牌蓝，**仅键盘导航显示**——`html[data-input-modality='pointer']`
  下非可编辑控件的 ring 置透明（按压仍保留背景反馈）。可编辑控件例外：点击也显示。
- **状态表达三板斧**：hover 半透明叠加 → active 更浓叠加 → disabled 40% 透明度。色相永不漂移。

## 8. Anti-Patterns（显式禁令）

1. **禁纯黑底**：深色画布是 `rgb(21,21,23)`（带蓝灰），`#000` 只允许做浅色模式的文字色值。
2. **禁多于一个强调色**：品牌蓝之外，红/绿/琥珀只做状态语义，永不装饰。
3. **禁色相漂移反馈**：hover/active 一律半透明叠加，不改基色。
4. **禁 ≥1px 实线边框**：分离一律 0.5px 发丝线（l1–l4 透明度阶梯），或画进 box-shadow。
5. **禁浓阴影/外发光**：投影透明度 ≤0.08；深色模式不靠投影靠描边。
6. **禁 display 切换显隐**：hover 显隐用 opacity，布局不可跳动。
7. **禁中间字重**：只用 400/500/600/700；Figma 510 落地写 500。
8. **禁裸 monospace 栈**：code 栈必须带 CJK 回退，防 Windows 宋体渗入。
9. **禁全圆角缺配对**：pill/圆点必须与 `corner-shape: round` 成对出现。
10. **禁浅色 Toast**：提示条永远深底浅字倒置。
11. **禁转圈 spinner**：加载一律骨架屏 + shimmer。
12. **禁悬停弹出焦点环**：指针输入下 ring 只留给键盘与可编辑控件。

## 9. Token 契约速查（迁移映射）

| dsh token | 语义 | ava2 对应（`--ava-*`） |
| --- | --- | --- |
| `--dsw-alias-bg-base` | 画布 | `--ava-bg-base` |
| `--dsw-alias-label-primary/secondary/tertiary` | 文字三档 | `--ava-label-*` |
| `--dsw-alias-border-l1–l4` | 发丝线四档 | `--ava-border-l1–l3`（缺 l4 可补） |
| `--dsw-alias-interactive-bg-hover/active` | 交互叠加 | `--ava-interactive-bg-hover` |
| `--dsw-alias-state-business-primary` | 品牌蓝 | `--ava-state-business-primary` |
| `--dsw-alias-state-error/success/warn-primary` | 状态主色 | `--ava-state-danger/success/warning` |
| `--dsw-radius-xs/sm/md/lg/xl/panel` | 圆角阶梯 | `--ava-radius-xs/sm/md/lg/xl/panel`（同值） |
| `--dsw-elevation-panel/prominent/soft` | 浮层三件套 | `--ava-shadow-elevated/panel` |
| `--ds-transition-duration(-fast/slow)` + ease | 动效节奏 | ava2 尚无，可按需引入 |

（dsh 源码锚点：`packages/client/ui-theme/src/styles/{base,design-platform,gradient-shadow-text,corner-shape,focus,scrollbar}.css`、`theme-settings.ts`，以及 `ui-primitives/src/{Button,Pill,Input,MenuSurface}.module.css`、`ui-chat/src/client/chat/MessageIconActions.module.css`。）
