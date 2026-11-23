---
title: AppHost 与 App 共享 design token 契约
description: 定义 ava2 宿主壳（AppHost）与各 App 共用的一套语义 design token（--ava-*），统一颜色、圆角、尺寸与字体来源，并规范 App 侧消费方式。
keywords: design token, --ava, 主题, dark theme, AppHost, chatapp, 样式统一, dsh
author: AI 助手
date: 2026-11-09
status: done
category: design
---

# AppHost 与 App 共享 design token 契约

## 1. 目标与原则

- **单一来源**：所有语义令牌只在一处定义 —— [tokens.css](file:///c:/workspace/SuperAI4/ava2/src/styles/tokens.css)，宿主与 App 一律 `var()` 消费。
- **一套命名**：统一 `--ava-<语义>` 前缀，语义对齐 dsh web ui 的 alias 令牌（`bg-*` / `label-*` / `border-*` / `interactive-*` / `state-*`）。
- **两套取值**：亮色在 `:root`，暗色在 `:root[data-theme="dark"]`；主题由 `documentElement` 的 `data-theme` 驱动，子树继承。
- **业务色不进契约**：气泡、代码块底色等 App 专有色彩留在 App 本地令牌内，避免污染公共语义。

## 2. 令牌清单

| 令牌 | 亮色 | 暗色 | 用途 |
| --- | --- | --- | --- |
| `--ava-bg-base` | `#ffffff` | `#151517` | 应用底色 |
| `--ava-bg-layer-1` | `#ffffff` | `#232324` | 一级面/卡片 |
| `--ava-bg-layer-2` | `#ffffff` | `#2c2c2e` | 二级面（输入、浮层） |
| `--ava-bg-sidebar` | `#f9fafb` | `#1b1b1c` | 侧边栏底色 |
| `--ava-label-primary` | `#0f1115` | `#f9fafb` | 主文本 |
| `--ava-label-secondary` | `#61666b` | `#cfd3d6` | 次文本 |
| `--ava-label-tertiary` | `#81858c` | `#adb2b8` | 辅助/占位文本 |
| `--ava-border-l1` | `rgba(0,0,0,.04)` | `rgba(255,255,255,.06)` | 最浅分隔 |
| `--ava-border-l2` | `rgba(0,0,0,.1)` | `rgba(255,255,255,.12)` | 常规边框/半像素分割线 |
| `--ava-border-l3` | `rgba(0,0,0,.12)` | `rgba(255,255,255,.16)` | 强调边框 |
| `--ava-interactive-bg-hover` | `rgba(38,49,72,.06)` | `rgba(255,255,255,.08)` | hover/选中底色 |
| `--ava-nav-active` | `#ebeef2` | `#43454a` | 导航/标签选中态 |
| `--ava-button-elevated-fill` | `#ffffff` | `#43454a` | 悬浮按钮（新建会话）底色 |
| `--ava-button-floating-hover` | `#f1f3f5` | `#353638` | 悬浮按钮 hover |
| `--ava-state-business-primary` | `#4176e6` | `#679efe` | 强调色/链接/运行态状态点 |
| `--ava-state-danger` | `#ec1313` | `#f25a5a` | 错误 |
| `--ava-state-success` | `#12a150` | `#3ecf8e` | 成功 |
| `--ava-state-warning` | `#b26a00` | `#f0b429` | 警告文本 |
| `--ava-brand` | `#0f1115` | `#f9fafb` | 品牌反色块 |
| `--ava-shadow-panel` | `0 8px 24px rgba(0,0,0,.08)` | `0 8px 28px rgba(0,0,0,.5)` | 面板阴影 |
| `--ava-shadow-elevated` | `0 2px 10px rgba(0,0,0,.06)` | `0 2px 14px rgba(0,0,0,.45)` | 悬浮控件阴影 |
| `--ava-space-0…5` | `2/4/8/10/16/24px` | 同 | 间隔阶梯（见 2.1） |
| `--ava-radius-xs/sm/md/lg/xl/panel/pill` | `4/10/12/16/20/28/999px` | 同 | 圆角阶梯（见 2.2） |
| `--ava-tabbar-h` | `40px` | 同 | 顶部合并条 / 画布头高度 |
| `--ava-font-ui` / `--ava-font-mono` | 系统字体栈 | 同 | 字体族 |

### 2.1 间隔阶梯

参照走查基准（EmberAI 暗色仪表盘）确立：**两级主间距 = 10px（元素内联） / 16px（沟槽）**，辅以 2/4/8 微间距与 24 分组留白，共六档，禁止档外裸值：

| 令牌 | 值 | 语义 |
| --- | --- | --- |
| `--ava-space-0` | `2px` | 微间距：紧凑行组（tabbar 页签间、表单行列间隙） |
| `--ava-space-1` | `4px` | 列表行间、按钮纵向内边距 |
| `--ava-space-2` | `8px` | 控件间距、小控件内边距 |
| `--ava-space-3` | `10px` | 元素内联：图标↔文字、输入区横向内边距 |
| `--ava-space-4` | `16px` | 沟槽：页面外边距、卡片间距、卡片内边距 |
| `--ava-space-5` | `24px` | 分组/区块留白、空态内边距 |

### 2.2 圆角阶梯

| 令牌 | 值 | 语义 |
| --- | --- | --- |
| `--ava-radius-xs` | `4px` | 微型控件（≤24px 方块：关闭钮、图标钮、标题徽标） |
| `--ava-radius-sm` | `10px` | 输入框、按钮、页签 |
| `--ava-radius-md` | `12px` | 列表选中项 / 导航项 |
| `--ava-radius-lg` | `16px` | 卡片、弹层、浮层 |
| `--ava-radius-xl` | `20px` | 大卡片 |
| `--ava-radius-panel` | `28px` | 大容器 |
| `--ava-radius-pill` | `999px` | 徽标 / 胶囊（计数、tag）；圆形元素仍用 `50%` |

## 3. 消费方式

### 3.1 宿主壳（AppHost）

`src/App.css` 顶部以 `@import "./styles/tokens.css";` 引入契约，其余规则只写 `var(--ava-*)`，不再定义任何色值/圆角。

### 3.2 App（如 chatapp）

App 产物以 ESM 形式运行在宿主文档内，可直接继承 `:root` 上的令牌。约定做法是**本地别名 + 宿主回退值**，这样样式正文保持稳定、宿主缺令牌时仍可读：

```css
.cc-root {
  --cc-t1: var(--ava-label-primary, #0f1115);
  --cc-l2: var(--ava-border-l2, rgba(0, 0, 0, 0.1));
  --cc-hover: var(--ava-interactive-bg-hover, rgba(38, 49, 72, 0.06));
  /* …业务专有令牌仍写本地字面量（气泡色、代码块底色等） */
}
```

暗色不再重复声明语义令牌，只需覆盖 App 专有色彩：

```css
[data-theme="dark"] .cc-root {
  --cc-bubble: #2c2c2e;
  --cc-code-bg: #1b1b1c;
}
```

## 4. 约束

- 禁止在 `App.css` / App 样式里新增与契约重名的裸色值；需要新语义时先在 `tokens.css` 增补。
- **间隔与圆角同样只允许消费阶梯令牌**（带字面回退）：禁止档外裸值（6/12/14px 等），归档规则见 2.1 / 2.2。
- **CSS 注释内禁止出现 `*/` 字符序列**（例如「apps 斜杠 src」这类路径缩写），否则注释会提前闭合，后续规则被 CSS 解析器整段丢弃（本次排查即因此导致亮色 `:root` 规则被丢弃）。
- App 容器 `.cc-root` 内不设 `background`，由宿主提供底色；App 只负责内容层。
- 状态点等小元素沿用 dsh 规格：`live` 会话 10px 圆点置于 16px 槽内，颜色取 `--ava-state-business-primary`。

## 5. 验证要点

改动令牌后应在 `http://localhost:1420/` 确认：

1. `getComputedStyle(document.documentElement).getPropertyValue('--ava-label-primary')` 亮/暗分别为 `#0f1115` / `#f9fafb`；
2. 间隔/圆角阶梯生效：`getPropertyValue('--ava-space-3')` = `10px`、`getPropertyValue('--ava-radius-sm')` = `10px`；
3. 侧边栏底色 `#f9fafb` / `#1b1b1c`，内边距 `8px 10px`；
4. chatapp「新建会话」按钮底色 `#ffffff` / `#43454a`，会话行文字色随主题切换；
5. 运行中会话的蓝色状态点（`live`）在明暗两种主题下均可见。
