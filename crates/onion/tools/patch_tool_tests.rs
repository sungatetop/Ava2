//! `patch_tool` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::ApplyPatchTool;
use crate::fs_observation::{version_of, FsObservation, Observed};
use crate::sandbox::Sandbox;
use crate::tools::ToolPort;
use crate::types::ToolCall;

/// 独立临时沙箱（测试结束清理）。
fn temp_sandbox(name: &str) -> (Arc<Sandbox>, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("onion_patch_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    (Arc::new(Sandbox::new(root.clone())), root)
}

fn call(input: &str) -> ToolCall {
    ToolCall { id: "c1".to_string(), name: "apply_patch".to_string(), input: json!({ "input": input }) }
}

/// 预置观测：把文件标记为已读（read-before-write 门禁）。
fn observe(observation: &FsObservation, root: &std::path::Path, relative: &str) {
    let data = std::fs::read(root.join(relative)).expect("read fixture");
    observation.record(relative, Observed::Present { version: version_of(&data) });
}

#[tokio::test]
async fn applies_add_update_delete_in_one_patch() {
    let (sandbox, root) = temp_sandbox("multi");
    std::fs::write(root.join("b.txt"), "context\nold\n").expect("fixture");
    let observation = Arc::new(FsObservation::new());
    observe(&observation, &root, "b.txt");
    let tool = ApplyPatchTool { sandbox: sandbox.clone(), observation };

    let patch = "\
*** Begin Patch
*** Add File: a.txt
+hello
*** Update File: b.txt
@@
 context
-old
+new
*** Delete File: c.txt
*** End Patch";
    std::fs::write(root.join("c.txt"), "gone\n").expect("fixture");
    observe(&tool.observation, &root, "c.txt");

    let result = tool.execute(call(patch), CancellationToken::new()).await;
    let blocks = result.expect("patch should apply");
    let text = match &blocks[0] {
        crate::types::ContentBlock::Text { text } => text.clone(),
        other => panic!("expected text block, got {other:?}"),
    };
    assert!(text.contains("added a.txt"), "unexpected: {text}");
    assert!(text.contains("updated b.txt"), "unexpected: {text}");
    assert!(text.contains("deleted c.txt"), "unexpected: {text}");
    assert_eq!(std::fs::read_to_string(root.join("a.txt")).unwrap(), "hello\n");
    assert_eq!(std::fs::read_to_string(root.join("b.txt")).unwrap(), "context\nnew\n");
    assert!(!root.join("c.txt").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn syntax_error_changes_nothing() {
    let (sandbox, root) = temp_sandbox("syntax");
    std::fs::write(root.join("a.txt"), "keep\n").expect("fixture");
    let tool = ApplyPatchTool { sandbox, observation: Arc::new(FsObservation::new()) };

    let error = tool
        .execute(call("*** Begin Patch\n*** Add File: b.txt\nno-plus\n*** End Patch"), CancellationToken::new())
        .await
        .unwrap_err();
    assert!(error.contains("must start with +"), "unexpected: {error}");
    assert!(!root.join("b.txt").exists());
    assert_eq!(std::fs::read_to_string(root.join("a.txt")).unwrap(), "keep\n");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn preflight_failure_changes_nothing() {
    let (sandbox, root) = temp_sandbox("preflight");
    std::fs::write(root.join("a.txt"), "alpha\n").expect("fixture");
    std::fs::write(root.join("b.txt"), "beta\n").expect("fixture");
    let observation = Arc::new(FsObservation::new());
    observe(&observation, &root, "a.txt");
    observe(&observation, &root, "b.txt");
    let tool = ApplyPatchTool { sandbox, observation };

    // 第一个操作合法，第二个 hunk 失配 → 预检整体拒绝，a.txt 不得被改。
    let patch = "\
*** Begin Patch
*** Update File: a.txt
@@
-alpha
+ALPHA
*** Update File: b.txt
@@
-missing
+BETA
*** End Patch";
    let error = tool.execute(call(patch), CancellationToken::new()).await.unwrap_err();
    assert!(error.contains("did not match"), "unexpected: {error}");
    assert_eq!(std::fs::read_to_string(root.join("a.txt")).unwrap(), "alpha\n");
    assert_eq!(std::fs::read_to_string(root.join("b.txt")).unwrap(), "beta\n");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn unread_file_is_rejected_by_observation_gate() {
    let (sandbox, root) = temp_sandbox("unread");
    std::fs::write(root.join("a.txt"), "alpha\n").expect("fixture");
    let tool = ApplyPatchTool { sandbox, observation: Arc::new(FsObservation::new()) };

    let error = tool
        .execute(
            call("*** Begin Patch\n*** Update File: a.txt\n@@\n-alpha\n+ALPHA\n*** End Patch"),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(error.contains("FS_NOT_OBSERVED"), "unexpected: {error}");
    assert_eq!(std::fs::read_to_string(root.join("a.txt")).unwrap(), "alpha\n");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn add_existing_file_is_rejected() {
    let (sandbox, root) = temp_sandbox("add_existing");
    std::fs::write(root.join("a.txt"), "alpha\n").expect("fixture");
    let tool = ApplyPatchTool { sandbox, observation: Arc::new(FsObservation::new()) };

    let error = tool
        .execute(
            call("*** Begin Patch\n*** Add File: a.txt\n+beta\n*** End Patch"),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(error.contains("already exists"), "unexpected: {error}");
    assert_eq!(std::fs::read_to_string(root.join("a.txt")).unwrap(), "alpha\n");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn move_renames_file() {
    let (sandbox, root) = temp_sandbox("move");
    std::fs::write(root.join("old.txt"), "body\n").expect("fixture");
    let observation = Arc::new(FsObservation::new());
    observe(&observation, &root, "old.txt");
    let tool = ApplyPatchTool { sandbox, observation };

    let patch = "\
*** Begin Patch
*** Update File: old.txt
*** Move to: new.txt
@@
 body
*** End Patch";
    tool.execute(call(patch), CancellationToken::new()).await.expect("move should apply");
    assert!(!root.join("old.txt").exists());
    assert_eq!(std::fs::read_to_string(root.join("new.txt")).unwrap(), "body\n");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn empty_patch_is_a_noop() {
    let (sandbox, root) = temp_sandbox("empty");
    let tool = ApplyPatchTool { sandbox, observation: Arc::new(FsObservation::new()) };
    let blocks = tool
        .execute(call("*** Begin Patch\n*** End Patch"), CancellationToken::new())
        .await
        .expect("empty patch ok");
    match &blocks[0] {
        crate::types::ContentBlock::Text { text } => assert!(text.contains("nothing to do")),
        other => panic!("expected text block, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&root);
}
