---
title: App 图标（AppIcon）设计规范
description: ava2 App 自定义图标的类型、绘制约束与使用约定：24×24 网格、描边/填充、单色、尺寸、命名与运行时校验。面向 App 开发者与宿主维护者。
keywords: AppIcon, 图标, SVG path, 设计规范, nav.sidebar, surface.canvas, 24×24
author: AI 助手
date: 2026-11-10
updated: 2026-11-10
status: done
category: references
---

# App 图标（AppIcon）设计规范

App 通过 slot 声明的 `icon` 字段向宿主提供图标，宿主统一渲染（不把尺寸/配色决策交给 App）。本规范定义 AppIcon 的**类型形态**与**绘制约束**，是 App 开发指南（`app-dev-guide.md`）的图标专项。

## 1. 类型形态

`AppIcon` 两种形态（类型权威：`src/host/types.ts`，slot 契约见 `contracts/app-contract.json`）：

```ts
type AppIcon =
  | string                                  // 单个 SVG path d
  | { d: string | string[]; fill?: boolean } // 多路径组合 / 填充模式
```

- **`string`**：单个 `path.d`。默认**描边**绘制（lucide 同源风格）。
- **`{ d }`**：`d` 可为单路径或多个路径的数组（logo、组合图形），数组内元素按声明顺序渲染为多个 `<path>`。
- **`{ d, fill: true }`**：按**填充**绘制（实心 logo / 品牌标记）。`fill` 缺省为 `false`（描边）。

示例：

```ts
// 描边单路径（lucide 同源）
ctx.slots.inject("nav.sidebar", {
  viewId: "mydocs", title: "文档", order: 3,
  icon: "M14 3v4a2 2 0 0 0 2 2h4 M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7z",
});

// 多路径组合（描边）
ctx.slots.inject("surface.canvas", {
  viewId: "graph", title: "图谱",
  icon: { d: ["M3 12h6", "M15 12h6", "M12 3v6", "M12 15v6"] }, // 十字准星
});

// 填充 logo（实心品牌标记）
ctx.slots.inject("nav.sidebar", {
  viewId: "brand", title: "品牌台", order: 1,
  icon: { d: "M12 2L2 12h4v10h6V12h4v10h6V12h4z", fill: true },
});
```

## 2. 坐标网格

- **viewBox 固定 `0 0 24 24`**，宿主按 24×24 网格渲染；App 不得改 viewBox。
- 图形须落在 **1×1 网格**上（对齐整数），关键轮廓优先贴 **2px 网格**，保证 18px 与 14px 两种渲染尺寸下不发糊。
- 视觉重心保持在网格中心（12,12），四周留 ≥1 单位的安全边距。

## 3. 描边 vs 填充

| 模式 | 适用 | 绘制规则 |
| --- | --- | --- |
| 描边（默认） | 功能图标、导航项 | 宿主统一 `stroke-width 1.8`（24 网格内固定值，等比缩放下视觉粗细随尺寸一致）、`round` cap/join；path 以线稿呈现，**禁止自带 stroke 属性** |
| 填充（`fill: true`） | logo、品牌标记、状态徽标 | 宿主设 `fill=currentColor`、`stroke=none`；path 须是闭合区域，可多路径嵌套（外轮廓 + 镂空） |

- 两种模式**不可混用**于同一图标（要么全部描边、要么全部填充）。
- 描边图标禁止在 path 内写死 `M...Z` 闭合填充面积（如空心圆用 `<circle>` 语义时只画描边）。

## 4. 单色与主题

- 图标一律使用 **`currentColor`**（宿主以 `fill`/`stroke` 继承当前文字色），**禁止**在 path 或属性中写固定色值（`#xxx`、`rgb()`、`red` 等）。
- 配色跟随宿主主题（亮/暗）与状态（激活、悬停、禁用），由宿主的 design token 决定；App 只提供**形状**，不提供颜色。
- 需要品牌色时，在 App 自己的组件内（而非 icon 字段）按 token 表达。

## 5. 尺寸

- 图标像素尺寸由**宿主**决定，App 不指定。统一渲染器为 `src/host/app-icon.tsx` 的 `AppIconView`，
  尺寸阶梯四档（常量 `ICON_SM / ICON_MD / ICON_LG / ICON_XL`）：

| 档位 | 尺寸 | 适用场景 | 容器 |
| --- | --- | --- | --- |
| `ICON_SM` | **14px** | 行内小图标（搜索框、App 组件内自绘） | — |
| `ICON_MD` | **18px** | 侧边导航（`nav.sidebar`）、画布 tab（`surface.canvas`）、标题栏与工具按钮 | 18px（nav-ico）/ 24px（icon-btn） |
| `ICON_LG` | **28px** | 应用市场网格 tile、详情 modal（64px / 56px 容器） | `.tile-icon` / `.drawer-icon` |
| `ICON_XL` | **36px** | 画布启动器卡片（72px 容器，图标占比约 50%） | `.launch-tile .tile-icon` |

- 新增渲染场景**必须**取自该阶梯，禁止临时硬编码任意像素值；容器与图标的比例约定在 45%–55% 区间。
- 宿主按 24×24 网格等比缩放，path 坐标与像素无关（1 单位 ≈ 0.75px@18）。

## 6. 命名约定

- **来源优先 lucide 同源**：能从 lucide 图标库取到的语义图标，直接复制其 `path.d`，不自行造线稿。
- path 命名无语义要求（`d` 是几何数据），但 App 的 `title` / `viewId` 应语义化，图标与语义一致（如 `docs` → 文档、`graph` → 图谱）。
- 多路径数组内元素按「底层 → 顶层」顺序声明（先底后上，避免遮挡错误）。

## 7. 运行时校验（宿主）

`registry.inject` 对 `nav.sidebar` / `surface.canvas` 的 `icon` 做**合法性校验**（`isValidAppIcon`）：

- `string`：非空；
- `{ d }`：`d` 为非空 string，或非空 string 数组（逐元素非空）；
- 其余形态（数字、布尔、空对象、缺失 `d`）→ `console.warn` 告警并**降级为无图标**（不阻断 App 装配）。

校验只保证形状合法，不校验 path 是否在 24×24 网格内——越界图形由人工评审把关。

## 8. 维护约定

- 修改 AppIcon 类型须同步：`src/host/types.ts`（类型权威）→ `contracts/app-contract.json`（slot 描述）→ 本规范 → `npm run check:contracts` 回归。
- 宿主渲染实现集中在 `src/host/app-icon.tsx` 的 `AppIconView`（尺寸阶梯常量同文件）；App 组件内自绘小图标时，也应复用同一套 24×24 网格与 token 规则。
- 新图标落地前检查亮/暗主题下对比度与激活态视觉一致性。
