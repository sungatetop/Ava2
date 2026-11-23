//! 溢出保留策略（post-execute 钩子）：估算 token 超过
//! 阈值的工具结果，在落 durable 之前把原文（文本块拼接 + 图片占位行）落
//! spill 工件，并把落盘内容替换为「头尾保留 + 溢出提示」；图片块原样保留
//! （多模态输入不损失）。spill 失败或未配置阈值时原文直通——策略永不丢结果。

use crate::compaction::estimate::estimate_blocks;
use crate::compaction::prune::{PRUNE_HEAD_CHARS, PRUNE_TAIL_CHARS};
use crate::types::{ContentBlock, ToolCall, ToolResult};

use super::store::SpillStore;

/// 头尾保留字符数：对齐 E1 视图级裁剪的 head/tail 尺寸——保留版总量始终低于
/// E1 的视图阈值，装配期确定性裁剪不会二次截断溢出提示。
const RETAIN_HEAD_CHARS: usize = PRUNE_HEAD_CHARS;
const RETAIN_TAIL_CHARS: usize = PRUNE_TAIL_CHARS;

/// 图片占位行中 URL 的预览长度（data URL 可能巨大，只留可辨识前缀）。
const IMAGE_URL_PREVIEW_CHARS: usize = 120;

/// 保留决策入口：估算 ≤ 阈值（或阈值 0）原样返回；否则落工件并返回保留版。
pub fn retain_tool_result(
    store: &SpillStore,
    owner_session_id: &str,
    call: &ToolCall,
    result: ToolResult,
) -> ToolResult {
    let threshold = store.max_inline_tokens();
    if threshold == 0 || estimate_blocks(&result.content) <= threshold {
        return result;
    }
    let full_text = render_full_text(&result.content);
    // 建议名不带扩展名：save_text 统一追加 `.txt` 后缀。
    let suggested_name = call.name.clone();
    let saved = match store.save_text(owner_session_id, &suggested_name, &full_text) {
        Ok(saved) => saved,
        // spill 失败（IO/权限）：保留原文直通——策略永不丢结果。
        Err(_) => return result,
    };
    let total_chars = full_text.chars().count();
    let omitted = total_chars.saturating_sub(RETAIN_HEAD_CHARS + RETAIN_TAIL_CHARS);
    let retained = format!(
        "{head}\n\n[... {omitted} chars omitted — full formatted result stored at: {locator}. {hint} ...]\n\n{tail}",
        head = head_text(&full_text),
        locator = saved.locator,
        hint = saved.retrieval_hint,
        tail = tail_text(&full_text),
    );
    let mut content = vec![ContentBlock::Text { text: retained }];
    // 图片块原样保留（spill 保留版仍携带图片本体）。
    content.extend(
        result
            .content
            .iter()
            .filter(|block| matches!(block, ContentBlock::Image { .. }))
            .cloned(),
    );
    ToolResult {
        tool_call_id: result.tool_call_id,
        name: result.name,
        is_error: result.is_error,
        content,
    }
}

/// 渲染工件全文：文本块原样（空行分隔），图片块渲染为占位行（可读投影；
/// 图片本体仍保留在结果内容里，见 [`retain_tool_result`]）。
fn render_full_text(content: &[ContentBlock]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for block in content {
        match block {
            ContentBlock::Text { text } => parts.push(text.clone()),
            ContentBlock::Image { image_url } => {
                let preview: String = image_url.chars().take(IMAGE_URL_PREVIEW_CHARS).collect();
                let ellipsis = if image_url.chars().count() > IMAGE_URL_PREVIEW_CHARS { "…" } else { "" };
                parts.push(format!("[Image: {preview}{ellipsis}]"));
            }
            // Thinking / ToolUse 不出现在工具结果内容中；出现也忽略。
            _ => {}
        }
    }
    parts.join("\n\n")
}

fn head_text(text: &str) -> String {
    text.chars().take(RETAIN_HEAD_CHARS).collect()
}

fn tail_text(text: &str) -> String {
    let total = text.chars().count();
    if total <= RETAIN_HEAD_CHARS {
        return String::new();
    }
    text.chars().skip(total.saturating_sub(RETAIN_TAIL_CHARS)).collect()
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
