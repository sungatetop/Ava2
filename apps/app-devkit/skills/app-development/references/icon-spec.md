---
title: App 图标（AppIcon）绘制指南
description: 面向 App 开发者的图标指南：AppIcon 类型形态与写法、24×24 网格绘制约束、描边与填充规则、单色约定、尺寸行为与运行时校验。宿主负责渲染与配色，App 只提供形状。
keywords: AppIcon, 图标, SVG path, 24×24, lucide, 描边, 填充
---

<!-- 本指南随 app-devkit 技能分发，面向 App 开发者编写（自包含、可移植）。
     类型权威为仓库 src/host/types.ts 的 AppIcon；宿主渲染细节见仓库
     src/host/app-icon.tsx（App 开发无需读）。 -->

# App 图标（AppIcon）绘制指南

App 通过 manifest `icon` 字段与 slot 声明的 `icon` 向宿主提供图标，宿主统一渲染——**尺寸、配色、主题适配全部由宿主决定，App 只提供形状**。

## 1. 类型与写法

`AppIcon` 两种形态（随技能分发的 `references/app-contract.json` slot 段有同源描述）：

```ts
type AppIcon =
  | string                                  // 单个 SVG path d
  | { d: string | string[]; fill?: boolean } // 多路径组合 / 填充模式
```

- **`string`**：单个 `path.d`，默认**描边**绘制（lucide 同源风格）。
- **`{ d }`**：`d` 为单路径或路径数组（logo、组合图形），数组内按声明顺序渲染为多个 `<path>`。
- **`{ d, fill: true }`**：按**填充**绘制（实心 logo / 品牌标记）。`fill` 缺省为 `false`（描边）。

示例：

```ts
// 描边单路径（lucide 同源）
ctx.slots.inject("nav.sidebar", {
  viewId: "mydocs", title: "文档", order: 3,
  icon: "M14 3v4a2 2 0 0 0 2 2h4 M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7z",
});

// 多路径组合（描边，十字准星）
ctx.slots.inject("surface.canvas", {
  viewId: "graph", title: "图谱",
  icon: { d: ["M3 12h6", "M15 12h6", "M12 3v6", "M12 15v6"] },
});

// 填充 logo（实心品牌标记）
ctx.slots.inject("nav.sidebar", {
  viewId: "brand", title: "品牌台", order: 1,
  icon: { d: "M12 2L2 12h4v10h6V12h4v10h6V12h4z", fill: true },
});
```

manifest 顶层 `icon` 写法相同（应用市场与详情抽屉用）：

```json
{ "icon": "M4 5h7 M4 10h9 M4 15h6 M17 4v6 M17 14v6 M14 7h6 M14 17h6" }
```

## 2. 坐标网格（绘制约束）

- **viewBox 固定 `0 0 24 24`**，宿主按 24×24 网格渲染；App 不得改 viewBox。
- 图形落在 **1px 网格**上（对齐整数），关键轮廓优先贴 **2px 网格**——保证 14px 与 18px 两种渲染尺寸下不发糊。
- 视觉重心保持在网格中心 (12,12)，四周留 ≥1 单位安全边距。

## 3. 描边 vs 填充

| 模式 | 适用 | 渲染规则（宿主执行） |
| --- | --- | --- |
| 描边（默认） | 功能图标、导航项 | 宿主统一 `stroke-width 1.8`、`round` cap/join；path 以线稿呈现，**path 内禁止自带 stroke 属性** |
| 填充（`fill: true`） | logo、品牌标记、状态徽标 | 宿主设 `fill=currentColor`、`stroke=none`；path 须是闭合区域，可多路径嵌套（外轮廓 + 镂空） |

- 同一图标**不可混用**两种模式（要么全描边、要么全填充）。
- 描边图标禁止在 path 内写死 `M...Z` 闭合填充面积。

## 4. 单色约定

- 图标一律使用 **`currentColor`**（宿主继承当前文字色渲染），**禁止**在 path 或属性中写固定色值（`#xxx`、`rgb()`、颜色名）。
- 配色跟随主题（亮/暗）与状态（激活、悬停、禁用），由宿主 design token 决定——App 只给形状。
- 需要品牌色时，在 App 自己的组件内（而非 icon 字段）按令牌表达。

## 5. 尺寸行为

- 像素尺寸由**宿主**按场景决定，App 不指定：行内小图标 14px、导航与工具按钮 18px、市场 tile 28px、画布启动器 36px；宿主按 24×24 网格等比缩放。
- App 组件内自绘小图标时，同样从 14 / 18 / 28 / 36 四档取值（容器与图标比例约 45%–55%），禁止临时硬编码任意像素值。
- path 坐标与像素无关（1 单位 ≈ 0.75px@18px）。

## 6. 图形来源与语义

- **来源优先 lucide 同源**：lucide 图标库已有的语义图标直接复制其 `path.d`，不自行造线稿（风格与宿主一致）。
- 图标与语义一致（`docs` → 文档、`graph` → 图谱）；App 的 `title` / `viewId` 语义化。
- 多路径数组按「底层 → 顶层」顺序声明，避免遮挡错误。

## 7. 运行时校验

宿主对 `manifest.icon` 与 `nav.sidebar` / `surface.canvas` 的 `icon` 做合法性校验：

- `string` 非空，或 `{ d }` 的 `d` 为非空字符串 / 非空字符串数组 → 通过；
- 其余形态（数字、布尔、空对象、缺 `d`）→ 宿主 `console.warn` 告警并**降级为无图标**（不阻断装配）。

校验只保证形状合法，不校验是否在 24×24 网格内——越界图形靠人工评审把关，请按 §2 自查。

## 8. 交付自查

- [ ] 14px 与 18px 两种尺寸下图形清晰（2px 网格对齐、线稿无过密细节）
- [ ] 亮 / 暗主题下对比度与激活态视觉一致（单色 currentColor 自然满足）
- [ ] 描边 / 填充模式未混用；path 无自带颜色属性
- [ ] lucide 同源优先，语义与图标一致
