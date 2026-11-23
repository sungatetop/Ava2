---
title: ava2 界面设计评审
description: 对 ava2 宿主壳与四个 App（chatapp / file-explorer / file-viewer / sessionboard）的界面设计做整体评审：设计语言一致性、排版、色彩、动效、可访问性，按 P0/P1/P2 给出改进项与快赢清单。
keywords: ava2, UI 评审, design tokens, dsh web ui, 设计一致性, 动效, 可访问性
author: AI 助手
date: 2026-11-10
status: done
category: reports
---

# ava2 界面设计评审

评审对象：宿主壳（`src/App.css` + `src/styles/tokens.css`）、chatapp（`.cc-*`）、file-explorer（`.fe-*`）、file-viewer（`.fv-*`）、sessionboard。参照系：项目自定的对齐目标 dsh web ui（alias 令牌 + 半像素边线 + 克制灰阶）。

## 一、设计体系盘点

| 维度 | 现状 | 评价 |
| --- | --- | --- |
| 令牌契约 | `--ava-*` 五族 alias（bg/label/border/interactive/state）+ space(2/4/8/10/16/24) + radius(4/10/12/16/20/28/pill) + 双字体栈 + 滚动条令牌，亮/暗成套 | 完整、纪律严明，全库 `var()` 消费，App 样式限域（`.cc-/.fe-/.fv-`），未见裸色值扩散 |
| 布局骨架 | 36px 标题栏 + 三栏 grid（可拖拽侧栏 264–420/折叠 56）+ 40px tabbar + 画布第二舞台（可拖宽 + 独立文件树列） | 结构清晰，min-width:0/min-height:0 防溢出到位 |
| 排版 | 基准 14/24，辅助 13/12，字重 500/600/700 节制；等宽栈含 NSimSun/SimHei 中文兜底 | 层级清楚；等宽场景（会话 id、代码）处理成熟 |
| 质感细节 | 0.5px 半像素边线三级、5px 全圆角滚动条皮肤（WebKit + Firefox 双路径）、resize 手柄 hover 显色 | 「机械细节」完成度高，对齐 dsh |
| 动效 | 仅宽度过渡（0.15–0.18s ease）与 ctx-bar 200ms | 明显偏薄（见 P1-3） |

## 二、强项

1. **令牌纪律是全项目最大资产**——五族 alias 语义清晰（`nav-active` 与 `interactive-bg-hover` 职责分离），亮暗两套不是简单反色（暗色 label 三级、border 三级独立校准），后续迭代不会漂移。
2. **克制的「安静」气质**——极浅 hover（黑 6%/白 8%）、半像素线、白底层次，与 dsh 的桌面级工具气质一致，没有 AI 味的渐变紫和圆角滥用。
3. **间隔/圆角双阶梯全覆盖**——2→24px 间隔、4→28px 圆角均有语义注释（元素内联/沟槽/分组留白；微控件/输入框/卡片/大容器），换算有依据。
4. **字体方案踩坑经验固化**——HarmonyOS Sans SC 前置 + 明确排除 system-ui/PingFang 的拦截问题，等宽中文兜底链完整。

## 三、问题清单

### P0（影响第一印象与可用性）

- **P0-1 强调色叙事缺失，界面没有记忆点**。`--ava-state-business-primary`（#4176e6）目前只出现在链接与 resize 手柄上。所有选中态（`.tab.on`、`.nav-link.on`）都是同一灰底，发送按钮、会话当前项、启动器图标全部无色。dsh 的辨识度来自「大面积安静 + 少量精准的蓝」。建议：发送/主行动按钮、当前会话左侧 2px 指示条、canvas:home 启动器图标 hover、交付物卡片左侧强调线，四处理智引入强调色即可扭转「灰蒙蒙」。
- **P0-2 键盘焦点态为零**。全库无 `:focus-visible`。桌面工具键鼠混用场景下 Tab 导航完全不可见。快赢：全局一行 `:focus-visible { outline: 2px solid var(--ava-state-business-primary); outline-offset: 2px; }` + 按需微调。
- **P0-3 空态是「灰字一句」**。`.empty-inline` / `.cc-empty` / `.fv-empty` / 启动器空态均为 tertiary 色纯文字，无图形无行动引导。新用户第一眼看到的就是这些空态，观感是「没做完」。建议沉淀统一空态组件：居中图标（AppIcon 复用）+ 一句人话 + 主行动按钮（如「新建会话」「打开文件树」）。

### P1（一致性 / 质感）

- **P1-1 动效只有宽度过渡**。tab 切换、画布第二舞台展开、AlertDialog 弹出、消息进入均无过渡。桌面工具不需要花哨，但弹出层缺 scale+fade（120ms）会显得「生硬跳变」。优先级：AlertDialog/picker 弹出 > canvas-side 展开 > 消息 stagger。
- **P1-2 tab 选中态与 hover 区分度不足**。`.tab.on` 与 `.nav-link.on` 同用 `--ava-nav-active` 灰底，28px 高的 tab 里 hover/选中几乎一样。建议选中 tab 增加 `font-weight: 500`（nav 已有，tab 没有）+ 底边 2px 强调线（或图标染强调色）。
- **P1-3 图标未收口**。宿主已有 AppIcon 统一出口（lucide 同源 24 网格），但 tab 关闭钮是字符 `×`（13px 文字），刷新按钮是文字「刷新」，各 App 文字按钮与图标按钮混用。建议：所有小控件图标化（X、RefreshCw、Play 等），文字按钮仅保留主行动。
- **P1-4 `--ava-brand` 语义混用**。brand=#0f1115（亮色下与 label-primary 同值），唯一使用点是 18px 标题栏徽标。黑徽标无品牌感且暗色下反转成白块。建议：徽标改强调蓝底或「蓝→青」极淡渐变，brand 令牌回归真实品牌色。
- **P1-5 canvas-panels 无拖拽调宽**。主侧栏有 264–420 拖拽，画布面板列固定 300px。功能不对称，长期会积累「为什么这边不能调」的抱怨。

### P2（打磨）

- **P2-1 danger 红过饱和**。#ec1313 在白底上非常刺眼，且目前唯一用途是删除确认。建议删除按钮常态 secondary 色、hover 才转 danger，红只在「不可逆」瞬间出现。
- **P2-2 侧栏层次偏平**。#f9fafb 与 #ffffff 对比极弱，三栏全靠 0.5px 线支撑；低分屏半像素线渲染成灰线后层次打折。可选：侧栏底色降半档或叠 1% 顶光渐变。
- **P2-3 状态色家族重量不均**。亮色下 danger(#ec1313)/success(#12a150) 高饱和，warning(#b26a00) 偏暗——同一行三个状态点视觉重量不一。微调 warning 至 #c47f00 一档即可。
- **P2-4 tertiary 信息的等宽运用不彻底**。会话 id、时间戳、路径 tooltip 等机械信息建议统一 `--ava-font-mono`，增强「工具感」。

## 四、快赢清单（半天内可全部落地）

1. 全局 `:focus-visible`（P0-2）——一行 CSS。
2. 发送按钮 + 当前会话指示条引入强调色（P0-1 的最小版本）。
3. 统一空态组件（图标 + 文案 + 行动按钮）（P0-3）。
4. AlertDialog/picker 弹出加 120ms scale+fade（P1-1 最小版）。
5. tab 选中加字重 + 强调线（P1-2）。
6. 删除按钮 hover 才转红（P2-1）。

## 五、总评

**7.5 / 10**。纪律与一致性达到 dsh 水准（令牌契约、半像素质感、滚动条/手柄等细节甚至有超出处），短板集中在「设计表达」：无强调色叙事、无动效编排、空态与焦点态缺位——是「工程师把规范执行得很好，但还没有设计师介入」的典型状态。快赢清单落地后预计可达 8.5：安静的工具底子 + 一处克制的品牌蓝 + 三处关键动效，即是这个产品该有的样子。
