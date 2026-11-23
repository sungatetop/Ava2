---
title: chatapp 对话图片附件（上传 → 发送 → 回放）
description: 对话 App 支持在 composer 添加/粘贴图片附件并随消息发送：AgentChannel.send 扩展 attachments、宿主组装 REST file parts、用户气泡渲染缩略图与 durable 回放，含真实模型识图冒烟结论与边界（仅图片、4MB/6 张上限）。
keywords: chatapp, 附件, 图片, 上传, MessageAttachment, send parts, 多模态, 回放, AgentChannel
author: AI 助手
date: 2026-11-12
status: done
category: solutions
---

# chatapp 对话图片附件（上传 → 发送 → 回放）

## 1. 背景与契约前提

chatapp 的 composer 此前只能发文本（`AgentChannel.send(id, text)` 固定拼 `parts:[text]`），用户无法把截图/图片交给模型。

服务端 REST 契约（`crates/onion-server/src/rest/dto.rs`）：

```rust
// POST /v1/sessions/{id}/messages 的 ContentPart
Self::File { file } => onion::types::ContentBlock::Image {
    image_url: file.uri.clone().unwrap_or_default(),
},
```

即 **`file` part 一律映射为 `Image` 内容块**，`uri` 必须是 `data:<mime>;base64,<payload>`（或可访问 URL）。因此：

- **图片是当前唯一真实可用的附件类型**（走多模态视觉链路）；
- 非图片文件若强行作为 `file` part 发出，会被当成图片 URL 送进模型请求，必然失败 → 前端**明确拒绝**而非静默误发。

图片 base64 随 durable `user_message`（`SessionEvent::UserMessage { message: AgentMessage }`）落盘，故刷新/重放后仍可还原缩略图，无需额外附件存储。

## 2. 契约扩展（host SDK）

[`src/host/types.ts`](../../src/host/types.ts) 新增：

```ts
export interface MessageAttachment {
  name: string;      // 展示名（文件名）
  mimeType: string;  // image/png | image/jpeg | image/webp | image/gif
  dataUrl: string;   // data:image/...;base64,...
}

send(id: string, text: string, attachments?: MessageAttachment[]): Promise<SendReceipt>;
```

[`src/host/agent.ts`](../../src/host/agent.ts) 组装 parts（**文本在前、附件在后**）：

```ts
const parts = [];
if (text) parts.push({ kind: "text", text });
for (const a of attachments) {
  parts.push({ kind: "file", file: { name: a.name, mimeType: a.mimeType, uri: a.dataUrl } });
}
if (parts.length === 0) parts.push({ kind: "text", text: "" });
```

该改动为**追加式**（第三参数可选），旧调用点 `send(id, text)` 行为不变。

## 3. UI 行为

| 位置 | 行为 |
| --- | --- |
| composer 工具条最左 | 28px 图片按钮（`.cc-attach-btn`），点开系统文件选择器（`accept="image/*"` `multiple`） |
| 输入框粘贴 | `onPaste` 取 `clipboardData.files`，截图可直接粘贴，拦截默认文本粘贴 |
| 输入框上方 | 待发送预览条 `.cc-attach-list`：32px 缩略图 + 文件名 + 移除按钮；超出上限时提示并截断 |
| 发送键 | `canSend = !busy && (文本非空 || 附件非空)`——**纯图片消息可发** |
| 用户气泡 | `.cc-user-images` 缩略图（≤220×220、圆角）在上，文本在下；纯图片消息不渲染空文本节点 |
| 发送后 | 输入区附件清空；会话列表在首条消息落盘后才刷新（惰性创建不变） |
| 发送失败 | 乐观气泡回滚 + **附件放回输入区**，避免用户重选 |

限制：单张 ≤ 4MB、单条 ≤ 6 张（base64 会整体落盘进 transcript，过大既撑请求又胀日志）。非 `image/*` 类型报「仅支持图片附件：<文件名>」。

纯图片消息对斜杠命令的交互：文本以 `/` 开头**且无附件**才走命令路径；带附件时按普通消息发送（命令不消费附件）。

## 4. 渲染与回放

`frame-reducer.ts` 的 `user_message` 分支新增 `imageBlocks()`：把 `blocks` 中 `type === "image"` 的 `image_url` 转为附件项（mime 从 data URL 前缀推导）。

两处配套修正：

1. 原来 `if (!text) return items` 会**丢弃纯图片消息**，改为「文本与图片都为空才跳过」；
2. 乐观气泡对账条件从「文本相同」改为「**文本相同且附件数相同**」，避免同文本连发时误转正上一条。

服务端 `ContentBlock::Image` 不持久化文件名，故回放态 `name` 为空串，UI 回退显示「图片」。

## 5. 验收

`npm run check`（chatapp + ava2 根）0 错；`npm run build` 产出 dist 并 `POST /v1/apps/install`（v0.9.6）。

playwright 冒烟 `ava2-verify/attach-smoke.mjs`（onion-server 9800 + Vite 1420，**真实模型 Qwen3.8-27B 识图**）**10/10 PASS**：

1. composer 有图片附件按钮；2. 选中图片出现预览 chip；3. 仅有附件时发送键可用；4. chip 可移除并重新添加；5. 用户气泡渲染 `data:image/png;base64,` 缩略图；6. **模型识别出图片中的 `AVA2-VISION`**（视觉链路打通）；7. 发送后输入区附件清空；8. 刷新回放仍显示图片（durable 落盘）；9. 非图片附件明确拒绝（`仅支持图片附件：note.txt`）；10. 无页面 JS 异常。

回归：发送键双态冒烟 3/3 PASS（空闲发送 → 生成中停止 → 停止后恢复）。

截图：`.screenshots/ava2-host/08-attach-sent.png`（气泡含图 + 助手回答）、`08-attach-replay.png`（刷新后回放）。

## 6. 已知边界

- **仅图片**：受 `file → Image` 映射限制。若需 PDF/文本附件，需扩展服务端 `ContentPart` 语义（如 `text/*` 内联为 Text 块），属独立设计项。
- **文件名不落盘**：durable 只保存 `image_url`，回放无原始文件名。
- **transcript 体积**：图片 base64 完整进 session JSONL；大图会显著增大日志（4MB 上限即为此设）。
- **npm 包版本**：`@ava-ai/app-sdk@0.1.0` 早于本次契约扩展，未含 `MessageAttachment`；对外发布需 bump 至 `0.1.1` 并重新 `npm publish`（仓库内 app 走 workspace 链接，不受影响）。
