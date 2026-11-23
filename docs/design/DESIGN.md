---
title: DESIGN.md — ava2 AppHost 设计系统
description: ava2 宿主壳与所有 App 的唯一设计基准：以 dsh 设计语言为参照系，从 tokens.css / App.css 源码提炼的语义令牌、字体阶梯、间隔圆角、阴影与交互纪律。
keywords: ava2, AppHost, 设计系统, design token, DESIGN.md, 字体阶梯, 交互规范
author: AI 助手
date: 2026-11-11
updated: 2026-11-11
status: done
category: design
---

# Design System: ava2 AppHost

> ava2 的**唯一设计基准**。形态与谱系承自 [dsh 设计系统](../references/DESIGN.md)（参照系），
> 但全部数值**提取自 ava2 源码实测**——`src/styles/tokens.css`（令牌权威）、`src/App.css`（壳组件）、
> `index.html`（字体加载）——非估算值。与 dsh 的差异在第 9 节差异表中显式登记。
>
> 生成或改造 ava2 的任何 UI（壳组件或 App）前，**先读本文件**；令牌一律 `var(--ava-*)` 消费，
> 禁止另起裸色值。

## 1. Visual Theme & Atmosphere

**「多舞台的工具型工作台」**——ava2 是 AppHost 桌面壳：36px 标题栏 + 三栏骨架（侧栏 →
tab 主区 → 画布第二舞台），App 以 slot 注入舞台。视觉气质继承 dsh 的克制传统：蓝灰中性
基底 + 单一品牌蓝，交互反馈走**半透明填充与透明度**而非色相漂移，边界用 **0.5px 发丝线**，
阴影极淡、深色模式层次分离靠描边承担。

- **Density：6/10**（每日工具偏紧凑：24–44px 控件高度，tab 与列表行信息密度高）
- **Variance：2/10**（严格对称的三栏结构化布局，无装饰性构图）
- **Motion：4/10**（轻快安静：0.12–0.2s 标准 ease，只动 fill/opacity/color 与 6px 内位移）

ava2 相对 dsh 的两处气质增量：**pill 全圆形态比例更高**（弹窗按钮/搜索框/开关/tag 全线
999px 圆），以及**主操作按钮 = 品牌蓝实底白字**（dsh 主按钮是中性黑）。

## 2. Color Palette & Roles

单层语义令牌：`src/styles/tokens.css` 定义 `--ava-<语义>` 的亮暗两套值，主题由
`documentElement` 的 `data-theme="light|dark"` 整体翻转，子树继承。壳与 App 一律 `var()`
消费；**App 业务专有色彩（气泡、代码块底色等）不进契约**，保留在 App 本地令牌内
（如 `apps/chatapp/src/styles.ts`）。

### 2.1 表面与文字（浅色 → 深色）

- **Canvas Base** `#ffffff` → `#151517` — 全局画布（深底带灰非纯黑）
- **Layer 1** `#ffffff` → `#232324` — 浮层一级（上浮菜单、输入底、悬浮窗内容）
- **Layer 2** `#ffffff` → `#2c2c2e` — 浮层二级（居中弹窗）
- **Sidebar Fill** `#f9fafb` → `#1b1b1c` — 侧栏专用底色（比画布低半档）
- **Ink Primary** `#0f1115` → `#f9fafb` — 主文字
- **Ink Secondary** `#61666b` → `#cfd3d6` — 次要文字、描述、ghost 控件字色
- **Ink Tertiary** `#81858c` → `#adb2b8` — 辅助文字、图标静息色、占位
- **Hairline L1–L3** `rgba(0,0,0,0.04/0.10/0.12)` → `rgba(255,255,255,0.06/0.12/0.16)`
  — 0.5px 发丝边框三档透明度阶梯（分隔线用 l1，控件边框用 l2，侧栏右缘用 l3）

### 2.2 唯一品牌强调色（Accent ≤ 1）

- **Business Blue** `#4176e6`（浅）/ `#679efe`（深）— 品牌蓝：主按钮实底、focus 描边、
  选中下划线、拖拽手柄高亮、用户头像底。**hover 档** `#5585ea`（浅，提亮）/ `#5b8ef0`
  （深，加深）——跟随 dsh「浅色 hover 提亮、深色 hover 加深」方向。深色模式提亮一档
  保证暗面可读。
- **Brand Neutral** `#0f1115` → `#f9fafb` — 中性品牌色（**黑/白随主题翻转**）：仅用于
  titlebar 徽标（黑底白字）。主操作一律品牌蓝，中性黑不做按钮底。

### 2.3 状态色（只做语义，不做装饰）

- **Success** `#12a150` / 深 `#3ecf8e` — 启用开关、状态绿点、tag.active
- **Danger** `#ec1313` / 深 `#f25a5a` — 错误文字、失败 tag；危险按钮**常态不显红**，
  hover 才转 danger（见 §6）
- **Warning** `#b26a00` / 深 `#f0b429` — 警示（浅色用深琥珀保文字对比度）
- **Idle** `--ava-label-tertiary` — 未激活/离线状态点

### 2.4 交互填充（核心机制：透明度叠加，不改色相）

- **Hover Wash** `rgba(38,49,72,0.06)` / 深 `rgba(255,255,255,0.08)` — 所有可 hover
  表面的统一叠加（导航、tab、菜单项、图标钮）
- **Nav Active** `#ebeef2` / 深 `#43454a` — 选中导航项/tab 的实底（比 wash 浓一档的灰）
- **Button Elevated** `#ffffff` / 深 `#43454a` — 浮起按钮填充
- **Floating Hover** `#f1f3f5` / 深 `#353638` — 浮起按钮 hover
- **Backdrop** `rgba(0,0,0,0.35)` — 模态遮罩（唯一全屏暗化）

## 3. Typography Rules

双字体栈，主题无关（亮暗同栈）：

- **UI Sans**：`"HarmonyOS Sans SC" → "Microsoft YaHei" → "Segoe UI" → "Helvetica Neue" → arial`
  HarmonyOS Sans SC 经 index.html **CDN 非阻塞加载**（`media="print"` 换轨），首绘走系统
  回退。**禁前置 `system-ui` / `PingFang SC` 等会提前拦截中文字形的字体**——否则目标字体
  永不生效（ava-app 踩坑结论）。
- **Code Mono**：`ui-monospace → "SF Mono" → "Cascadia Mono" → "JetBrains Mono" → consolas
  → menlo → "DejaVu Sans Mono" → "NSimSun" → "SimHei"`。**末尾 NSimSun/SimHei 为中文等宽
  兜底**——缺了它，中文混排（会话 id、代码注释）会回退非等宽中文字体，宽度参差。

### 3.1 字号阶梯（root 基准 14px/24px，壳组件实测档位）

| 字号/行高 | 字重 | 用途 |
| --- | --- | --- |
| 11/18 | 400 | 极小标签下限：tag、code、user 栏副标题 |
| 11/14 | 600 | 分组标题（user-pop-group-title） |
| 12/18 | 400 | 次要信息：dim、时间戳、悬浮窗标题、遮罩说明 |
| 13/18–24 | 400–600 | **界面默认档**：按钮、tab、导航、菜单项、tile 名 |
| 14/— | 400–500 | nav-link（14px 导航档） |
| 15/— | 600 | 确认弹窗标题 |
| 16/24 | 600 | 抽屉主名 |
| 18/— | 700 | 页面大标题（store-head h1，字距 -0.2px） |

### 3.2 字重纪律

只允许 **400 / 500 / 600 / 700** 四档；新代码禁中间值（存量 650 待收敛）。

## 4. Spacing, Radius & Shape

### 4.1 间距：显式 token 六档（ava2 签名，dsh 无）

ava2 **把间距收敛为令牌**，组件一律 `var(--ava-space-N)`：

| 令牌 | 值 | 语义分工 |
| --- | --- | --- |
| `--ava-space-0` | 2px | 微缝（tab 间隙、分组行距） |
| `--ava-space-1` | 4px | 元素内最小间距 |
| `--ava-space-2` | 8px | 控件内边距、面板间 gap |
| `--ava-space-3` | 10px | **元素内联**（图标↔文字、标题栏 padding、控件间隙） |
| `--ava-space-4` | 16px | **沟槽**（页面外边距、卡片间距、分区间） |
| `--ava-space-5` | 24px | **分组留白**（弹窗内边距、网格行距） |

例外字面值（6px 弹层内边距、3px 开关 knob 偏移等）仅限微调场景，不新增档位。

### 4.2 控件高度阶梯

| 高度 | 用途 |
| --- | --- |
| 16px | tab 关闭钮 |
| 18px | 图标格（nav-ico/user-pop-ico）、appbar 徽标 |
| 24px | icon-btn、tag（pill）、开关 |
| 28px | tab、消息操作行 |
| 32px | 导航行、上浮菜单项、悬浮窗标题栏 |
| 34px | 市场搜索框/安装按钮（pill） |
| 38px | 弹窗操作按钮 |
| 40px | **顶部合并条（appbar）** / 画布头（`--ava-tabbar-h`） |
| 44px | user 栏 |

### 4.3 圆角阶梯

`xs 4 / sm 10 / md 12 / lg 16 / xl 20 / panel 28 / pill 999`。控件取 sm–md，列表选中项与
导航取 md，卡片与浮层取 lg，大图标容器取 xl。**pill（999px）全圆是 ava2 的高频形态**：
弹窗按钮、搜索框、开关、tag、tab 下划线、滚动条 thumb 全线全圆。

两条硬规则：
1. **微控件用 xs**：16px 以下的小操作钮（tab-close、tab-float）取 radius-xs(4)，否则圆角
   吃掉可点面积观感。
2. **全圆组件不配平滑圆角机制**：ava2 未引入 dsh 的 corner-shape superellipse，全圆直接
   `border-radius: var(--ava-radius-pill)`，无引擎差异问题。

### 4.4 结构尺寸（壳骨架常量）

- 侧栏：默认 280px，拖拽 264–420px，折叠 56px（图标-only）
- 画布面板列：固定 300px（可收 24px 窄条）
- 悬浮窗：`min(640px, 72vw) × min(560px, 66vh)`
- tab 选中下划线：16×2px，强调色

## 5. Elevation & Shadow

两档单层柔影（dsh 三件套在 ava2 简化为两档）：

- **Elevated** `0 2px 10px rgba(0,0,0,0.06)` / 深 `0 2px 14px rgba(0,0,0,0.45)` —
  启动器 tile 图标、悬浮窗（轻浮起）
- **Panel** `0 8px 24px rgba(0,0,0,0.08)` / 深 `0 8px 28px rgba(0,0,0,0.5)` —
  居中弹窗、上浮菜单（重浮起）

配套纪律：
- 浮层一律 **0.5px 发丝描边 + 柔影并存**；深色模式投影变浓但层次分离仍由描边兜底。
- **z-index 阶梯**：画布手柄 5 → 悬浮窗 20 → 上浮菜单 30 → 宿主模态 40。新增浮层按序插入，
  禁跳级。
- ava2 未采用 dsh「0.5px 画进 box-shadow」手法——直接真 0.5px border（不占布局问题在
  ava2 尚未出现，保持简单）。

## 6. Component Stylings

- **appbar（顶部合并条）**：40px 单行、0.5px l2 底边，横向分四段——
  **身份区**（18px 徽标贴左 + 13px/600 名称 + 7px 状态点 + 24px 折叠 icon-btn，折叠钮贴
  分隔线）→ **0.5px l3 竖分隔** → **tab 列表**（`flex: 0 1 auto` + `overflow: hidden`，
  多 tab 不挤走右侧按钮）→ spacer → **画布开关**（贴右，右距 10px）。
  **身份区宽度 = 侧栏宽度**（行内注入 `--ava-sidebar-w`，`width: calc(... - 10px)`），
  分隔线正落在侧栏右缘、tab 起点与下方主区列对齐；侧栏拖拽变宽时同步跟随。侧栏折叠到
  56px 图标轨时收起名称与状态点，只留徽标 + 折叠钮。**不展示后端端点文字**——连接状态
  由状态点颜色表达。原 titlebar(36px) 与 tabbar(40px) 两行已合一，省 36px 纵向空间。
- **nav-link**：32px 高、radius-md、图标 18px；hover = Hover Wash + 文字提亮；
  active = Nav Active 实底 + 字重 500。
- **tab**：28px 高、radius-sm；hover = Wash；选中 = Nav Active 实底 + **底部 16×2px
  强调色短下划线**（与 hover 灰底拉开区分，ava2 签名）。tab 列表 `flex: 0 1 auto` +
  `overflow-x: auto`（滚动条隐藏，滚轮纵向手势映射为横向），单个 tab `flex: 0 0 auto`
  不被压缩——多 App 时靠滚动而非挤压。标题 `max-width: 160px` + 省略号。
- **tab 悬浮按钮**：`position: absolute` 落在关闭钮左侧的内隙，**常态不占宽度**，
  `opacity: 0` + `pointer-events: none`；`.tab:hover` 或已悬浮（`.on`）时淡入。
  代价：hover 瞬间会压住标题末字约 16px（与 Chrome 关标签覆盖标题尾同构）。
- **icon-btn**：24px 方块、radius-xs、静息 secondary 色；hover = Wash + 提亮；active
  实底同 nav。
- **user 栏 + 上浮菜单**：44px 底栏、28px 圆头像（品牌蓝底白字首字母）；弹层 fixed
  248px、radius-md、0.5px 描边 + Panel 阴影、`translateY(6px)` 入场 0.12s；菜单项
  32px/radius-sm；App 分组标题 11px 大写字距条目。
- **按钮三形态**（弹窗操作行 38px pill）：
  - `btn-solid`：品牌蓝实底白字，hover 切 `--ava-state-business-primary-hover` 档
    （浅提亮 / 深加深，非 wash 手法——实底色块用专用 hover 档）；
  - `btn-ghost`：0.5px 描边 + 透明底，hover = Wash；
  - `btn-danger`：**常态弱化为 secondary 外观，hover 才转红**（`rgba(236,19,19,0.08)` 底 +
    danger 字）——红只在不可逆操作瞬间出现，ava2 签名交互。
- **默认 button 元素**（工具性小按钮）：13px、radius-sm、0.5px l2 发丝边框、hover 边框转
  tertiary（带 0.15s 过渡）；disabled `opacity 0.45`。
- **输入框**：0.5px l2 描边、radius-sm/置 pill、`bg-layer-1/2`；focus 描边变品牌蓝
  （不是 focus ring）；placeholder 走 tertiary 档。
- **tag**：pill、11px、0.5px l2 描边、18px 行高；active/failed 换状态色字+边框。
- **switch**：40×24 pill、白 knob 18px、on = Success 绿实底、0.2s 滑动。
- **modal**：`layer-2` 底、radius-lg、0.5px 描边 + Panel 阴影、`scale(0.96)+translateY(6px)`
  入场 0.18s；遮罩 0.35 黑。
- **启动器 tile**：72px 图标容器（radius-xl、hover 渐变底 + 强调色图标）、hover 上浮
  `translateY(-2px)`（ava2 唯一位移动效）；市场 tile 同构 64px，禁用态灰度 + 0.55 透明。
- **悬浮窗 stage-overlay**：fixed 卡片**无遮罩**（主界面仍可操作）、32px 拖拽头
  （`cursor: grab/grabbing`）、右下 16px 斜纹 grip 缩放手柄（hover 转强调色）。
- **Scrollbar**：5px 细条、透明轨道、pill thumb 走 `--ava-scrollbar-thumb`（hover 加深）；
  WebKit 伪元素 + Firefox `@supports` 标准属性双路径互斥门控。

## 7. Motion & Interaction

- **节奏**：0.12s（弹层/悬浮钮显隐）→ 0.15s（侧栏宽度、按钮）→ 0.18s（画布/弹窗/tile）
  → 0.2s（开关），一律标准 `ease`。**无 transition-duration token、无 spring、无编排**——
  动效只动 fill/opacity/color 与 ≤6px 位移。
- **Hover 显隐**：tab 悬浮钮静息 `opacity: 0`，hover/active 态 `opacity: 1`（0.12s）——
  用 opacity 而非 display，布局零位移（对齐 dsh data-actions-reveal 思想）。
- **Focus Ring**：全局 `:focus-visible` = 2px 强调色 `outline` + 2px offset（键盘 Tab
  导航显示，鼠标点击由浏览器模态判定抑制）。ava2 未实现 dsh 的 `data-input-modality`
  显式指针抑制——依赖浏览器原生 focus-visible 行为，禁改用 `:focus` 全量显示。
- **状态表达三板斧**：hover 半透明叠加 → active 实底加深 → disabled 45% 透明度。
  色相永不漂移（实底色块用专用 hover 档，见 §2.2）。

## 8. Anti-Patterns（显式禁令）

1. **禁硬编码色值/字号**：壳与 App 一律 `var(--ava-*)`；App 业务色只进 App 本地令牌，
   不进 tokens.css。
2. **禁壳写业务色**：App.css 只承载布局与中性外观，业务外观归 App。
3. **禁多于一个强调色**：品牌蓝之外，红/绿/琥珀只做状态语义，永不装饰。
4. **禁色相漂移反馈**：hover/active 一律半透明叠加或实底档位，不改基色。
5. **禁 ≥1px 实线边框**：分离一律 0.5px 发丝线走 l1–l3 阶梯（含默认 button 元素）。
6. **禁纯黑**：深色画布是 `#151517`；`#000` 只允许出现在遮罩 rgba 中。
7. **禁字体栈前置 system-ui / PingFang SC**：会拦截中文字形使 HarmonyOS Sans 永不生效。
8. **禁 mono 栈删中文兜底**：NSimSun/SimHei 末坠保证中文等宽混排。
9. **禁中间字重**：只用 400/500/600/700。
10. **禁跳级 z-index**：新浮层按 5/20/30/40 阶梯插入。
11. **禁转圈 spinner**：加载用文案弱化或骨架占位，不引入重阴影与旋转动画。
12. **禁单主题交付**：新增样式必须亮/暗双态验证（截图各留一张）。

## 9. 与 dsh 参照系的差异表（决策记录）

ava2 有意偏离 dsh 的全部决策点，改动前先对照此表：

| 维度 | dsh | ava2 | 缘由 |
| --- | --- | --- | --- |
| 间距 | 无 token，4px 网格字面值 | **六档 `--ava-space-*` 显式令牌** | App 可注入消费，契约化 |
| 圆角 sm | 8px | **10px** | 输入框/按钮手感更圆润 |
| 阴影 | 三件套（描边画进 shadow） | 两档单层柔影 + 真 0.5px border | 实现简单，视觉等价 |
| 主按钮 | 中性黑底白字 | **品牌蓝实底白字**（中性黑只做徽标） | ava2 品牌识别 |
| pill 比例 | 控件少用 | 高频（按钮/搜索/开关/tag） | 桌面工作台亲和感 |
| 字体 | 系统栈（Segoe UI/PingFang） | **HarmonyOS Sans SC CDN + 雅黑兜底** | 对齐 ava-app 已验证方案 |
| mono 兜底 | CJK 回退 PingFang/雅黑 | **NSimSun/SimHei 中文等宽兜底** | 混排等宽需求 |
| Focus | data-input-modality 显式抑制 | 原生 `:focus-visible` | 壳层简单优先 |
| 平滑圆角 | corner-shape superellipse | 未引入 | 引擎兼容面收窄 |
| warning | label 琥珀 `#dd8629` | **`#b26a00`（浅）/`#f0b429`（深）** | 浅色文字对比度 |
| 发丝线档位 | l1–l4 | **l1–l3** | 未用到 l4，缺时再补 |
| 壳字号 | 默认 14 | root 14 / 组件主用 13 | 桌面密度 |

**存量不一致收敛记录（2026-11-11 全部完成）**：① 默认 button 1px 实线 → 0.5px 发丝线
（补 0.15s 过渡）；② `user-pop-group-title` 字重 650 → 600；③ `btn-solid` hover brightness
→ 专用 hover 档 `--ava-state-business-primary-hover`（新增 token）；④ `.canvas-resizer:hover`
未定义的 `--ava-state-focus` 引用 → `--ava-state-business-primary`。顺带收敛：
`confirm-desc` 12.5px → 13/20 档、`drawer-switch` disabled 0.5 → 0.45 统一。

## 10. Token 契约速查

| 令牌 | 语义 | 浅色 | 深色 |
| --- | --- | --- | --- |
| `--ava-bg-base` | 全局画布 | `#ffffff` | `#151517` |
| `--ava-bg-layer-1/2` | 浮层一/二级 | `#ffffff` | `#232324` / `#2c2c2e` |
| `--ava-bg-sidebar` | 侧栏底 | `#f9fafb` | `#1b1b1c` |
| `--ava-label-primary/secondary/tertiary` | 文字三档 | `#0f1115` / `#61666b` / `#81858c` | `#f9fafb` / `#cfd3d6` / `#adb2b8` |
| `--ava-border-l1/l2/l3` | 发丝线三档 | 黑 4%/10%/12% | 白 6%/12%/16% |
| `--ava-interactive-bg-hover` | hover 叠加 | `rgba(38,49,72,.06)` | `rgba(255,255,255,.08)` |
| `--ava-nav-active` | 选中实底 | `#ebeef2` | `#43454a` |
| `--ava-state-business-primary(-hover)` | 品牌蓝（+hover 档） | `#4176e6` / `#5585ea` | `#679efe` / `#5b8ef0` |
| `--ava-state-success/danger/warning` | 状态色 | `#12a150` / `#ec1313` / `#b26a00` | `#3ecf8e` / `#f25a5a` / `#f0b429` |
| `--ava-brand` | 中性徽标 | `#0f1115` | `#f9fafb` |
| `--ava-shadow-elevated/panel` | 两档阴影 | 2px/10px 6% · 8px/24px 8% | 2px/14px 45% · 8px/28px 50% |
| `--ava-scrollbar-thumb(-hover)` | 滚动条 | `#dde0e5` / `#c8cdd4` | `#45474d` / `#585b62` |
| `--ava-space-0…5` | 间距六档 | 2/4/8/10/16/24px | 同 |
| `--ava-radius-xs…panel/pill` | 圆角阶梯 | 4/10/12/16/20/28/999px | 同 |
| `--ava-tabbar-h` | 顶部合并条 / 画布头高度 | 40px | 同 |
| `--ava-font-ui / --ava-font-mono` | 字体栈 | 见 §3 | 同 |

（源码锚点：`src/styles/tokens.css` 令牌权威；`src/App.css` 壳组件规格；`index.html`
字体加载与 import map；App 本地令牌示例 `apps/chatapp/src/styles.ts`。）
