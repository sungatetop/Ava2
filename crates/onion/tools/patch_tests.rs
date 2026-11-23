//! `patch` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::{
    apply_hunks, parse_patch, ApplyHunksResult, HunkStage, PatchOperation,
};

fn update_hunks(patch: &str) -> Vec<super::PatchHunk> {
    let operations = parse_patch(patch).expect("parse patch");
    match operations.into_iter().next().expect("one operation") {
        PatchOperation::Update { hunks, .. } => hunks,
        other => panic!("expected update, got {other:?}"),
    }
}

#[test]
fn parses_add_update_delete_and_move() {
    let patch = "\
*** Begin Patch
*** Add File: a.txt
+hello
+world
*** Update File: b.txt
@@ fn main
 context
-old
+new
*** Update File: c.txt
*** Move to: d.txt
@@
 keep
*** Delete File: e.txt
*** End Patch";
    let operations = parse_patch(patch).expect("parse");
    assert_eq!(operations.len(), 4);
    match &operations[0] {
        PatchOperation::Add { path, content, .. } => {
            assert_eq!(path, "a.txt");
            assert_eq!(content, "hello\nworld\n");
        }
        other => panic!("expected add, got {other:?}"),
    }
    match &operations[1] {
        PatchOperation::Update { path, hunks, move_to, .. } => {
            assert_eq!(path, "b.txt");
            assert!(move_to.is_none());
            assert_eq!(hunks.len(), 1);
            assert_eq!(hunks[0].context.as_deref(), Some("fn main"));
            assert_eq!(hunks[0].old_lines, vec!["context".to_string(), "old".to_string()]);
            assert_eq!(hunks[0].new_lines, vec!["context".to_string(), "new".to_string()]);
        }
        other => panic!("expected update, got {other:?}"),
    }
    assert_eq!(operations[2].move_to(), Some("d.txt"));
    assert!(matches!(&operations[3], PatchOperation::Delete { path, .. } if path == "e.txt"));
}

#[test]
fn rejects_missing_begin_or_end() {
    let error = parse_patch("*** Add File: a.txt\n+x\n*** End Patch").unwrap_err();
    assert!(error.reason.contains("Begin Patch"), "unexpected: {error}");
    let error = parse_patch("*** Begin Patch\n*** Add File: a.txt\n+x").unwrap_err();
    assert!(error.reason.contains("End Patch"), "unexpected: {error}");
}

#[test]
fn rejects_update_without_hunks() {
    let error = parse_patch("*** Begin Patch\n*** Update File: a.txt\n*** End Patch").unwrap_err();
    assert!(error.reason.contains("at least one hunk"), "unexpected: {error}");
}

#[test]
fn rejects_add_content_without_plus_prefix() {
    let error = parse_patch("*** Begin Patch\n*** Add File: a.txt\nnope\n*** End Patch")
        .unwrap_err();
    assert!(error.reason.contains("must start with +"), "unexpected: {error}");
}

#[test]
fn applies_update_hunk_with_context_anchor() {
    let hunks = update_hunks(
        "*** Begin Patch\n*** Update File: a.txt\n@@ fn main\n context\n-old\n+new\n*** End Patch",
    );
    let before = "header\nfn main\ncontext\nold\ntail\n";
    match apply_hunks(before, &hunks) {
        ApplyHunksResult::Ok { after } => {
            assert_eq!(after, "header\nfn main\ncontext\nnew\ntail\n");
        }
        other => panic!("expected ok, got {other:?}"),
    }
}

#[test]
fn applies_multiple_hunks_in_patch_order() {
    let hunks = update_hunks(
        "*** Begin Patch\n*** Update File: a.txt\n@@\n-a\n+A\n@@\n-c\n+C\n*** End Patch",
    );
    match apply_hunks("a\nb\nc\n", &hunks) {
        ApplyHunksResult::Ok { after } => assert_eq!(after, "A\nb\nC\n"),
        other => panic!("expected ok, got {other:?}"),
    }
}

#[test]
fn unmatched_hunk_returns_failure_without_partial_text() {
    let hunks = update_hunks(
        "*** Begin Patch\n*** Update File: a.txt\n@@\n-missing\n+new\n*** End Patch",
    );
    match apply_hunks("a\nb\n", &hunks) {
        ApplyHunksResult::Failed { failure } => {
            assert_eq!(failure.stage, HunkStage::OldLines);
            assert_eq!(failure.pattern, vec!["missing".to_string()]);
        }
        other => panic!("expected failure, got {other:?}"),
    }
}

#[test]
fn unmatched_context_anchor_reports_context_stage() {
    let hunks = update_hunks(
        "*** Begin Patch\n*** Update File: a.txt\n@@ no such anchor\n x\n*** End Patch",
    );
    match apply_hunks("a\n", &hunks) {
        ApplyHunksResult::Failed { failure } => assert_eq!(failure.stage, HunkStage::Context),
        other => panic!("expected failure, got {other:?}"),
    }
}

#[test]
fn trailing_whitespace_differences_still_match() {
    let hunks = update_hunks(
        "*** Begin Patch\n*** Update File: a.txt\n@@\n-old   \n+new\n*** End Patch",
    );
    match apply_hunks("old\n", &hunks) {
        ApplyHunksResult::Ok { after } => assert_eq!(after, "new\n"),
        other => panic!("expected ok, got {other:?}"),
    }
}

#[test]
fn end_of_file_anchor_matches_tail_only() {
    let hunks = update_hunks(
        "*** Begin Patch\n*** Update File: a.txt\n@@\n-dup\n+last\n*** End of File\n*** End Patch",
    );
    // 两处 dup：EOF 锚点必须命中尾部那处。
    match apply_hunks("dup\nmid\ndup\n", &hunks) {
        ApplyHunksResult::Ok { after } => assert_eq!(after, "dup\nmid\nlast\n"),
        other => panic!("expected ok, got {other:?}"),
    }
}

#[test]
fn empty_patch_yields_no_operations() {
    assert!(parse_patch("*** Begin Patch\n*** End Patch").expect("parse").is_empty());
}
