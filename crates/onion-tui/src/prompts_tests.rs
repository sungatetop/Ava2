//! prompts 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use onion::ask_user::types::AskUserQuestion;

/// 红框卡结构断言（宽度由内容决定，不硬编码 pad）：
/// 首行 `╭─…─╮`、中间行 `│ … │`、末行 `╰─…─╯`，第 2 行为反色标题。
fn assert_box_shape(lines: &[Line]) {
    assert!(lines.len() >= 3, "至少框顶 + 标题 + 框底");
    assert!(lines[0].text.starts_with('╭') && lines[0].text.ends_with('╮'));
    assert!(lines[lines.len() - 1].text.starts_with('╰') && lines[lines.len() - 1].text.ends_with('╯'));
    for line in &lines[1..lines.len() - 1] {
        assert!(line.text.starts_with("│ ") && line.text.ends_with("│"), "{}", line.text);
        assert!(line.style.color == Some(crossterm::style::Color::Red), "{}", line.text);
    }
    // 标题行反色（卡片头）。
    assert!(lines[1].style.reverse, "标题行应反色");
}

#[test]
fn approval_panel_shows_name_input_and_options() {
    let panel = PromptPanel::new();
    let lines = panel.render_approval("Bash", &serde_json::json!({ "command": "ls -la" }));
    assert_box_shape(&lines);
    assert!(lines[1].text.contains("审批：Bash"));
    assert!(lines.iter().any(|l| l.text.contains("1. 允许  2. 拒绝（/approve /deny）")));
}

#[test]
fn plan_review_panel_shows_plan_and_options() {
    let panel = PromptPanel::new();
    let lines = panel.render_plan_review("第一步\n第二步");
    assert_box_shape(&lines);
    assert!(lines[1].text.contains("计划审批"));
    assert!(lines.iter().any(|l| l.text.contains("第一步")));
    assert!(lines.iter().any(|l| l.text.contains("第二步")));
    assert!(lines.iter().any(|l| l.text.contains("1. 批准  2. 继续规划（/approve /deny）")));
}

#[test]
fn batch_progress_renders_when_set() {
    let mut panel = PromptPanel::new();
    panel.batch = Some((2, 3));
    let lines = panel.render_approval("Edit", &serde_json::json!({}));
    assert!(lines[1].text.contains("审批（2/3）：Edit"));
}

#[test]
fn long_input_is_truncated_head_tail() {
    let long = "x".repeat(400);
    let panel = PromptPanel::new();
    let lines = panel.render_approval("Bash", &serde_json::json!({ "command": long }));
    let body = &lines[2];
    assert!(body.text.chars().count() < 210);
    assert!(body.text.contains('…'));
}

#[test]
fn ask_user_panel_lists_questions_and_numbered_options() {
    let questions = vec![AskUserQuestion {
        id: "mode".to_string(),
        question: "选哪种模式？".to_string(),
        header: None,
        options: Some(vec![
            onion::ask_user::types::AskUserOption { label: "chat".to_string(), description: Some("只读".to_string()) },
            onion::ask_user::types::AskUserOption { label: "work".to_string(), description: None },
        ]),
        multi_select: None,
    }];
    let panel = PromptPanel::new();
    let lines = panel.render_ask_user(&questions);
    assert_box_shape(&lines);
    assert!(lines[1].text.contains("待答复"));
    assert!(lines.iter().any(|l| l.text.contains("[mode] 选哪种模式？")));
    assert!(lines.iter().any(|l| l.text.contains("1. chat（只读）")));
    assert!(lines.iter().any(|l| l.text.contains("2. work")));
}
