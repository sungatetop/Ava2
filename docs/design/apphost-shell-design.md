---
title: AppHost 宿主壳布局设计
description: ava2 宿主壳（标题栏 / 侧边栏 / tab 化主区 / 右侧抽屉）的布局结构与 dsh web ui 对齐的设计令牌，供新增壳级控件与 App 视觉对齐参考。
keywords: AppHost, 宿主壳, 布局, dsh web ui, 设计令牌, 主题, tab
author: AI 助手
date: 2026-11-08
updated: 2026-11-11
status: done
category: design
---

# AppHost 宿主壳布局设计

宿主壳负责"装 App"的外层结构，不含任何业务视图。布局参考 `ava/prototypes/app-host-proto/src/Shell.tsx`（三栏 + tab 化主区），视觉与令牌对齐 dsh web ui，使预置 App（chatapp）与三方 App 在同一套色阶、圆角、字号下渲染。

## 1. 结构

```
app-root                     # 纵向：顶部合并条 + 三栏
├── appbar (40px)            # 身份区（18px 徽标贴左 + 名称 + 状态点 + 折叠开关，开关贴分隔线）
│                            #   身份区宽度 = 侧栏宽度（行内注入 --ava-sidebar-w，减掉 appbar 内边距）
│                            #   ｜0.5px l3 竖分隔（落在侧栏右缘）｜ tab 列表｜ spacer ｜ 画布开关
├── shell (grid: auto | 1fr | auto)
│   ├── sidebar              # nav-link 列表（App 贡献的 nav.sidebar + 壳自有「App 管理」）+ panel.sidebar
│   │                        #   宽度行内驱动：默认 280，拖拽 264–420，可折叠 56px（图标-only）
│   ├── main-area
│   │   └── stage            # surface.main（App 视图）或 App 管理面板；视图被悬浮时显示占位提示
│   └── canvas-side          # 画布第二舞台（420px，折叠仅收宽度）
│       ├── canvas-tabbar    # 画布 tab 编排（canvas:home 启动器常驻 + App 认领的画布 tab）
│       ├── canvas-stage     # canvas:home 启动器 或 surface.canvas（App 视图）
│       └── canvas-panels    # panel.canvas 面板区（跟随当前画布视图 viewId，无贡献时整区隐藏）
└── stage-overlay ×N         # 浮层舞台（多悬浮窗，fixed z-20）：标题栏（拖动/还原/关闭）+ MainSurface
                             #   + 右下角缩放手柄；数组顺序即层叠序，点击置顶（详见 app-floating-windows-design.md）
```

- **一切皆 App**：侧栏导航项全部来自 `registry.navItems()`（App 经 `slots.inject("nav.sidebar")` 贡献）；壳只额外提供 `__apps`（App 管理）一个自有视图，保证"壳与 App 同一契约"。
- **tab 化主区**：`openViews` 为打开列表（点击导航即打开），渲染期过滤掉已被禁用/卸载 App 的 tab（不在 effect 里剪枝，避免多余重渲染）。关闭当前 tab 则回到"无视图"态。
- **浮层舞台（多悬浮窗）**：tab 上的悬浮按钮把 `surface.main` 视图脱离 tab 悬浮于主界面之上——overlay 层直接复用 `MainSurface`（App 契约零改动），每窗独立拖动/缩放/还原/关闭，点击置顶，关 tab 连带收层。窗框归壳、内容归 App、是否悬浮归用户（App 不能强弹）。详见 [app-floating-windows-design.md](./app-floating-windows-design.md)。
- **侧栏交互（对齐 dsh / ava-app）**：右缘 4px 拖拽把手调宽（264–420，`body.resizing` 期间禁用文本选择），标题栏左侧 panel-left 按钮折叠为 56px 图标-only 轨（文字与侧栏面板隐藏）；宽度与折叠态均持久化到 `localStorage`。折叠态不渲染拖拽把手与侧栏面板。
- **画布第二舞台**（对齐原型 canvas-stage 设计）：
  - `canvas:home` 启动器 tab 常驻，枚举全部 `surface.canvas` 贡献的 App 卡片（图标 + 标题 + 描述），点击即打开画布 tab；
  - 画布 tab 独立于主区 tab 编排；与主区同构只渲染当前激活视图（tab 间切换/关闭即卸载——lazy-cache 仅驻留组件定义不驻留 state，需跨切换保留的状态由 App 经 `ctx.storage` 自管）；App 被禁用/卸载时对应 tab 自动隐去，失效兜底回启动器；
  - 折叠仅收宽度（右侧 panel-right 按钮开合），视图状态驻留；
  - App/Agent 经总线 `canvas.open { viewId }` 事件打开画布 tab（折叠中被打开自动展开）；
  - `panel.canvas`（`{ order, viewId, title, component }`）跟随当前画布视图渲染在舞台下方面板区，`order` 排序、按 viewId 过滤，无贡献时整区隐藏。
- **主题**：壳持有 `light | dark`，落 `document.documentElement.dataset.theme` 并写 `localStorage["ava2.theme"]`。App 只需消费 `var(--*)`，无需感知切换逻辑。

## 2. 设计令牌（`src/styles/tokens.css`）

| 组 | 变量 | 亮色 | 暗色 |
| --- | --- | --- | --- |
| 背景 | `--ava-bg-base` / `--ava-bg-layer-1` / `--ava-bg-layer-2` | `#fff` / `#fff` / `#fff` | `#151517` / `#232324` / `#2c2c2e` |
| 侧栏 | `--ava-bg-sidebar` | `#f9fafb` | `#1b1b1c` |
| 文字 | `--ava-label-primary` / `-secondary` / `-tertiary` | `#0f1115` / `#61666b` / `#81858c` | `#f9fafb` / `#cfd3d6` / `#adb2b8` |
| 边框 | `--ava-border-l1` / `-l2` / `-l3` | `.04` / `.10` / `.12` 黑 | `.06` / `.12` / `.16` 白 |
| 交互 | `--ava-interactive-bg-hover` / `--ava-nav-active` | `.06` 蓝灰 / `#ebeef2` | `.08` 白 / `#43454a` |
| 语义 | `--ava-brand` / `--ava-state-business-primary` / `-danger` / `-success` / `-warning` | `#0f1115` / `#4176e6` / `#ec1313` / `#12a150` / `#b26a00` | `#f9fafb` / `#679efe` / `#f25a5a` / `#3ecf8e` / `#f0b429` |
| 圆角 | `--ava-radius-sm\|md\|lg\|xl\|panel` | 10 / 12 / 16 / 20 / 28 | 同 |
| 尺寸 | `--ava-tabbar-h` | 40（顶部合并条 / 画布头；侧栏宽度不走令牌，行内驱动） | 同 |
| 字体 | `--ava-font-ui` / `--ava-font-mono` | 系统 UI 栈 / `SF Mono, JetBrains Mono, Fira Code, Consolas` | 同 |

约定：

- 语义色只表达状态（成功/失败/警告/业务强调），不用于装饰；强调色在 dsh 体系里是"墨色"（`--text-primary`），业务蓝仅用于主操作（如发送按钮）。
- 半像素边框（`0.5px solid var(--border-l*)`）是 dsh 的层次手段，禁止用阴影替代分隔线（抽屉浮层除外）。
- 圆角遵循 4/8/12/16/20/28 阶梯，不引入中间值。

## 3. 关键控件规格

| 控件 | 规格 |
| --- | --- |
| `.nav-link` | 高 32px、圆角 12、gap 10、内边距 0 8；选中仅背景 `--ava-nav-active`（无左侧指示条）；图标列固定 18px |
| `.tab` | 高 28px、圆角 10、字号 13、`flex: 0 0 auto`（不被压缩）；选中 `--ava-nav-active`；关闭按钮 16px 方块 |
| `.appbar-tabs` | `overflow-x: auto` + 隐藏滚动条，溢出靠横向滚动；滚轮纵向手势在 JSX 中映射为 `scrollLeft`；选中 tab 自动 `scrollIntoView` |
| `.tab-float` | 绝对定位在关闭钮左侧内隙，常态不占宽（`opacity:0` + `pointer-events:none`），hover / 已悬浮时淡入；标题 `max-width:160px` + 省略号 |
| `.icon-btn` | 24×24、悬停 `--ava-interactive-bg-hover` |
| `.sidebar` | `padding: 6px 8px`、`gap: 4px`、底色 `--ava-bg-sidebar`、右缘 `0.5px` `--ava-border-l3`；宽度行内驱动（280 默认 / 264–420 拖拽 / 56 折叠） |
| `.sidebar-resize` | 右缘 4px 命中区、`col-resize` 光标、悬停业务蓝 35% 透明 |
| `.appbar` | 高 40px（`--ava-tabbar-h`）、`padding: 0 10px`、底部 `0.5px` `--ava-border-l2`；徽标 18px 圆角 4 贴左；不展示后端端点文字 |
| `.appbar-lead` | 身份区容器，`width: calc(var(--ava-sidebar-w) - 10px)`（行内注入变量），使分隔线落在侧栏右缘；折叠态仅剩徽标 + 折叠钮（名称/状态点 `display:none`，`gap: 0`） |
| `.appbar-sep` | `0.5px × 16px` 竖分隔、`--ava-border-l3`，右缘与侧栏右缘齐平 |
| `.canvas-side` | 展开宽 420、左缘半像素；折叠 `width: 0` + overflow hidden（0.18s 过渡），视图不卸载 |

## 4. 与 App 的边界

- 壳只持布局与中性外观；App 自带业务样式，且必须限域在自身根类下（如 chatapp 的 `.cc-root`），避免污染壳与其他 App。
- App 的视觉令牌应引用壳定义的 `var(--*)` 或按同一套数值落局部变量，保证亮/暗主题一致。
- 壳不向 App 暴露"当前主题"的读写 API；App 只消费 CSS 变量的暗色覆盖即可。
- 画布契约：App 经 `slots.inject("surface.canvas", { viewId, title, description?, icon?, component })` 认领画布视图；打开走启动器卡片或总线 `canvas.open` 事件，App 不直接操作壳的 tab 状态。

## 5. 验证（2026-11-08）

- 编译级：`tsc --noEmit` 与 `vite build`（含 vendor 重打包）通过；hello 冒烟 App 重装后 `surfaces` 含 `surface.canvas`。
- 交互清单（浏览器 `http://localhost:1420/`，onion-server 9800）：appbar 折叠开关（56px 图标轨）、侧栏右缘拖拽（264–420）、appbar 右侧按钮开合画布、启动器打开「会话总览」、画布 tab 关闭回启动器、`canvas.open` 总线事件、暗色主题下三栏观感。
- 合并条验收（2026-11-11）：appbar 高 40、无残留 `.titlebar` / `.tabbar`、身份区与 tab 中线对齐、`.appbar-sep` 0.5×16、折叠侧栏后不横向溢出（10/10 PASS）。
- 身份区调整（2026-11-11）：移除 `.appbar-dim` 端点文字、徽标贴左（左距 10px）、折叠开关移入身份区右侧并紧贴分隔线（7/7 PASS，亮/暗/折叠三态截图留证）。
- 分隔线对齐（2026-11-11）：分隔线左缘 279.5 = 侧栏右缘 280，tab 起点 = 侧栏右缘 + 8；拖拽侧栏到 340 时同步跟随；折叠态分隔线落在 55.5/56 且身份区无溢出（7/7 PASS）。
