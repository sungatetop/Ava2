//! `spill::policy` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;

use super::retain_tool_result;
use crate::spill::store::SpillStore;
use crate::types::{ContentBlock, ToolCall, ToolResult};

fn temp_root(tag: &str) -> std::path::PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("onion_spill_policy_{tag}_{}_{}", std::process::id(), seq))
}

fn call(name: &str) -> ToolCall {
    ToolCall { id: "call-1".to_string(), name: name.to_string(), input: json!({}) }
}

fn text_of(result: &ToolResult) -> &str {
    match result.content.first() {
        Some(ContentBlock::Text { text }) => text,
        _ => unreachable!("首个块应为文本"),
    }
}

#[test]
fn retain_passes_through_below_threshold() {
    let root = temp_root("below");
    let store = SpillStore::new(root.clone(), 10_000);
    let original = ToolResult::text(&call("grep"), "small output");
    let retained = retain_tool_result(&store, "onion-1", &call("grep"), original.clone());
    assert_eq!(retained, original);
    // 未落任何工件。
    assert!(!root.join("sessions").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn retain_respects_zero_threshold_as_disabled() {
    let root = temp_root("disabled");
    let store = SpillStore::new(root.clone(), 0);
    let original = ToolResult::text(&call("grep"), "huge ".repeat(20_000));
    let retained = retain_tool_result(&store, "onion-1", &call("grep"), original.clone());
    assert_eq!(retained, original);
    assert!(!root.join("sessions").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn retain_spills_oversized_text_with_head_tail_and_notice() {
    let root = temp_root("spill");
    let store = SpillStore::new(root.clone(), 1);
    let body = "a".repeat(20_000);
    let original = ToolResult::text(&call("run_terminal"), format!("HEADMARK{body}TAILMARK"));
    let retained = retain_tool_result(&store, "onion-1", &call("run_terminal"), original);

    let text = text_of(&retained);
    assert!(text.starts_with("HEADMARK"));
    assert!(text.ends_with("TAILMARK"));
    assert!(text.contains("chars omitted"));
    assert!(text.contains("full formatted result stored at:"));
    // 保留版总量低于 E1 视图阈值（8KiB）：装配期裁剪不会二次截断溢出提示。
    assert!(text.len() < 8 * 1024);

    // 工件已落盘且含全文；工件名编码了工具名。
    let session_dir = root.join("sessions").join("onion-1");
    let entries: Vec<_> = fs::read_dir(&session_dir).unwrap().collect();
    assert_eq!(entries.len(), 1);
    let artifact = fs::read_to_string(entries[0].as_ref().unwrap().path()).unwrap();
    assert!(artifact.contains("HEADMARK"));
    assert!(artifact.contains("TAILMARK"));
    let file_name = entries[0].as_ref().unwrap().file_name();
    assert!(file_name.to_string_lossy().ends_with("run_terminal.txt"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn retain_preserves_image_blocks_and_renders_placeholder() {
    let root = temp_root("image");
    let store = SpillStore::new(root.clone(), 1);
    let call = call("read_image");
    let original = ToolResult {
        tool_call_id: call.id.clone(),
        name: call.name.clone(),
        is_error: false,
        content: vec![
            ContentBlock::Text { text: "a".repeat(20_000) },
            ContentBlock::Image { image_url: "file:///tmp/pic.png".to_string() },
        ],
    };
    let retained = retain_tool_result(&store, "onion-1", &call, original);

    // 保留版 = 文本保留块 + 原样图片块。
    assert_eq!(retained.content.len(), 2);
    assert!(matches!(retained.content[1], ContentBlock::Image { ref image_url } if image_url == "file:///tmp/pic.png"));
    // 工件全文含图片占位行。
    let session_dir = root.join("sessions").join("onion-1");
    let mut entries = fs::read_dir(&session_dir).unwrap();
    let artifact = fs::read_to_string(entries.next().unwrap().unwrap().path()).unwrap();
    assert!(artifact.contains("[Image: file:///tmp/pic.png]"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn retain_truncates_huge_image_urls_in_placeholder() {
    let root = temp_root("dataurl");
    let store = SpillStore::new(root.clone(), 1);
    let long_url = format!("data:image/png;base64,{}", "A".repeat(5_000));
    let call = call("read_image");
    let original = ToolResult {
        tool_call_id: call.id.clone(),
        name: call.name.clone(),
        is_error: false,
        content: vec![
            ContentBlock::Text { text: "a".repeat(20_000) },
            ContentBlock::Image { image_url: long_url.clone() },
        ],
    };
    let retained = retain_tool_result(&store, "onion-1", &call, original);
    let session_dir = root.join("sessions").join("onion-1");
    let artifact = fs::read_to_string(
        fs::read_dir(&session_dir).unwrap().next().unwrap().unwrap().path(),
    )
    .unwrap();
    // 占位行只保留前缀，不携带完整 data URL。
    assert!(artifact.contains("[Image: data:image/png;base64,"));
    assert!(!artifact.contains(&long_url));
    // 图片本体原样保留在结果内容里。
    assert!(
        matches!(retained.content[1], ContentBlock::Image { ref image_url } if *image_url == long_url)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn retain_falls_back_to_original_when_spill_fails() {
    // root 指向普通文件 → create_dir_all 失败 → 原文直通。
    let root = temp_root("fail");
    fs::write(&root, "not a directory").unwrap();
    let store = SpillStore::new(root.clone(), 1);
    let original = ToolResult::text(&call("grep"), "huge ".repeat(20_000));
    let retained = retain_tool_result(&store, "onion-1", &call("grep"), original.clone());
    assert_eq!(retained, original);
    let _ = fs::remove_dir_all(root);
}
