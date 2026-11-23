//! `tools` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::RalphTool;

#[test]
fn parse_report_recognizes_statuses() {
    let text = "STATUS: complete\nSUMMARY: done\nNEXT_STEPS: none\nBLOCKER: none";
    let report = RalphTool::parse_report(text);
    assert_eq!(report.status, "complete");
    assert_eq!(report.summary, "done");
}

#[test]
fn parse_report_normalizes_unknown_status_to_continue() {
    let text = "STATUS: maybe\nSUMMARY: x";
    let report = RalphTool::parse_report(text);
    assert_eq!(report.status, "continue");
}

#[test]
fn parse_report_caps_summary_length() {
    let long = "SUMMARY: ".to_string() + &"a".repeat(20_000);
    let report = RalphTool::parse_report(&long);
    assert!(report.summary.chars().count() <= 16_384);
}

#[test]
fn round_prompt_includes_objective_and_first_round_marker() {
    let prompt = RalphTool::round_prompt("build a thing", None);
    assert!(prompt.contains("build a thing"));
    assert!(prompt.contains("first round"));
}

#[test]
fn round_prompt_includes_prior_report() {
    let prompt = RalphTool::round_prompt("build a thing", Some("STATUS: continue\nSUMMARY: progress"));
    assert!(prompt.contains("previous round's report"));
    assert!(prompt.contains("progress"));
}
