---
title: App 悬浮舞台设计（浮层式 overlay）
description: ava2 AgentOS 的 App 悬浮 UI 最终决策：宿主加一个 overlay 层挂 MainSurface，App 主舞台悬浮于主界面之上；记录两轮被否方向（页面内模拟窗、OS 级多窗体）与最终浮层式的实现。
keywords: ava2, App 契约, 悬浮舞台, overlay, 多悬浮窗, MainSurface, AgentOS, 拖动, 缩放
author: AI 助手
date: 2026-11-10
updated: 2026-11-11
status: done
category: design
---

# App 悬浮舞台设计（浮层式 overlay）

## 1. 意图与决策演进

需求原点：App 是否支持悬浮 UI / 悬浮界面 / 拖动？（宿主 = 操作系统）

| 轮次 | 方向 | 结论 |
| --- | --- | --- |
| v1 | 页面内模拟窗：宿主自绘窗框（拖动/缩放/z-order/焦点栈），`ctx.windows.open` 命令式 API | **否**——本质是「舞台内越界的合法化」，自研窗框成本高 |
| v2 | OS 级多窗体：Tauri `WebviewWindow`，新窗体 = App 独占新舞台 | **否**——超出了当前需要，用户选择了更轻的形态 |
| v3（采纳） | **浮层式：宿主加一个 overlay 层挂 `MainSurface`**——App 主舞台悬浮于主界面之上 | **已实现** |

v3 的关键洞察：**悬浮的不是「新窗口」，而是「现有主舞台本身」**。`MainSurface` 是按 viewId 渲染 `surface.main` 贡献的独立组件，overlay 层直接复用它，App 契约零改动——任何已认领 `surface.main` 的 App 视图无需任何适配即可悬浮。

## 2. 实现（已落地）

壳层（`src/App.tsx`）：

- **入口**：主区 tab 上新增悬浮按钮（hover 出现，悬浮态常驻高亮；`__apps` 壳自有视图除外）
- **状态**：`overlays: OverlayWin[]`（多悬浮窗数组，`viewId` 唯一键 + 各自 `pos`/`size`）——每个视图可独立悬浮，不同 App / 同 App 不同视图互不影响
- **overlay 层**：`app-root` 末尾 `.map()` 渲染多个 `.stage-overlay` fixed 卡片——标题栏（拖动手柄 + 还原 + 关闭）+ `.stage-overlay-body` 挂 `<MainSurface viewId={win.viewId} />`（与 tab 舞台同构：lazy-cache / ErrorBoundary / Suspense 全部复用）；新窗位置按窗数级联错开（28px 步进）避免完全重叠
- **主区联动**：被悬浮视图的原 tab 保留，主区舞台显示占位提示；「还原」= 回 tab；「关闭视图」= 还原并关 tab（`closeView` 联动清对应窗）
- **拖动**：标题栏 Pointer Events + `setPointerCapture`（捕获后 move/up 持续派发到起始元素，光标快速移动/移出窗口不丢事件，WebView2 下比 window 监听稳定）+ 视口边界夹取；函数式更新按 `viewId` 定位条目，无闭包旧状态问题
- **缩放**：右下角 `.stage-overlay-resize` 手柄（同 Pointer Events 捕获套路），尺寸夹取 min 320×240 且不超视口（受该窗当前位置约束）；默认 640×560
- **置顶**：数组顺序即 DOM 顺序（后者叠最上）；点击窗体（`focusOverlay`）将该窗移到末尾置顶

样式（`src/App.css`）：`.tab-float` / `.stage-overlay` / `.stage-overlay-head` / `.stage-overlay-body` / `.stage-overlay-resize`，全部 token 化（亮暗主题跟随）。

层级契约：舞台内容 z-index 10 ＜ **浮层舞台 20** ＜ 宿主模态 40——浮层不遮罩（主界面仍可操作），也压不住宿主级对话框。

## 3. 机制 / 策略边界

| 职责 | 归属 |
| --- | --- |
| 悬浮入口、窗框（拖动/缩放/还原/关闭）、层级、生命周期（关 tab 连带收层） | 壳（宿主） |
| 舞台内全部 UI 形态与交互 | App（不变，弹窗边界 = 舞台边界，见 ui-gallery demo） |
| 悬浮哪些视图、何时悬浮 | 用户（宿主入口），App 无法强弹——保持宿主 UI 权威 |

## 4. 远期（不承诺）

- 位置/尺寸持久化
- `ctx.windows` 命令式 API（App 主动请求悬浮）与 OS 级多窗体（v2 方向，需求回归时再评估）
