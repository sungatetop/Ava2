//! `sandbox` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use super::Sandbox;
use crate::types::ContentBlock;

fn temp_root(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("ava_onion_sandbox_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap_or_else(|error| panic!("create temp root failed: {error}"));
    root
}

fn sandbox(tag: &str) -> (Sandbox, std::path::PathBuf) {
    let root = temp_root(tag);
    (Sandbox::new(root.clone()), root)
}

#[tokio::test]
async fn resolve_rejects_paths_outside_writable_roots() {
    let (sandbox, _root) = sandbox("resolve");
    // 可写根 = 权限根 + 系统临时目录（writableRoots）。测试根位于 temp
    // 之下：两层 `..` 落在 temp 内（合法），三层及以上才能越过 temp；越出后的
    // 最终位置一律拒绝（不看表述形式，看归一化后的最终位置）。
    let outside = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("outside_reject.txt");
    for input in [outside.to_str().unwrap(), "../../outside.txt", "a/../../../b.txt"] {
        let error = match sandbox.resolve(input) {
            Ok(path) => panic!("{input} should be rejected, got {}", path.display()),
            Err(error) => error,
        };
        assert!(
            error.contains("outside the permission root"),
            "{input} got: {error}"
        );
    }
}

#[tokio::test]
async fn resolve_accepts_absolute_path_and_parent_inside_root() {
    let (sandbox, root) = sandbox("abs_inside");
    // 根内绝对路径直接可用（工具返回值 / 宿主拼出的路径 / 模型复述的路径）。
    let absolute = root.join("a").join("b.txt");
    assert_eq!(sandbox.resolve(absolute.to_str().unwrap()), Ok(absolute));
    // `..` 允许，归一化后仍在根内即可。
    let nested = root.join("a").join("..").join("c.txt");
    assert_eq!(
        sandbox.resolve(nested.to_str().unwrap()),
        Ok(root.join("c.txt"))
    );
    assert_eq!(sandbox.resolve("a/../d.txt"), Ok(root.join("d.txt")));
}

#[tokio::test]
async fn resolve_joins_relative_path_under_root() {
    let (sandbox, root) = sandbox("join");
    let path = sandbox.resolve("a/b.txt").unwrap_or_else(|error| panic!("resolve failed: {error}"));
    assert_eq!(path, root.join("a").join("b.txt"));
}

#[tokio::test]
async fn write_creates_parent_dirs_and_read_roundtrip() {
    let (sandbox, root) = sandbox("roundtrip");
    let message = sandbox.write("deep/dir/note.txt", "hello sandbox").await;
    assert_eq!(
        message,
        Ok(format!("wrote 13 bytes to {}", root.join("deep").join("dir").join("note.txt").display()))
    );
    let text = sandbox.read_text("deep/dir/note.txt").await;
    assert_eq!(text, Ok("hello sandbox".to_string()));
}

#[tokio::test]
async fn read_text_rejects_images_and_points_to_read_image() {
    let (sandbox, _root) = sandbox("read_text_image_guard");
    std::fs::write(sandbox.root().join("dot.png"), b"x")
        .unwrap_or_else(|error| panic!("seed png failed: {error}"));
    let result = sandbox.read_text("dot.png").await;
    assert!(result.is_err(), "read must not silently return image bytes as text");
}

#[tokio::test]
async fn read_image_rejects_non_images() {
    let (sandbox, _root) = sandbox("read_image_text_guard");
    std::fs::write(sandbox.root().join("note.txt"), b"x")
        .unwrap_or_else(|error| panic!("seed txt failed: {error}"));
    let result = sandbox.read_image("note.txt").await;
    assert!(result.is_err(), "read_image must reject non-image files");
}

#[tokio::test]
async fn read_allows_outside_and_write_scoped_to_writable_roots() {
    // 读放开（读在所有模式下放行）：越界读成功（spill 日志 / 系统记忆
    // 场景）；写遏制落在可写根 = 权限根 + 系统临时目录（writableRoots）：
    // temp 可写，两者之外仍拒绝。
    let (sandbox, _root) = sandbox("read_escape");
    let outside = std::env::temp_dir().join("ava_onion_outside.txt");
    std::fs::write(&outside, b"secret").unwrap_or_else(|error| panic!("seed outside file failed: {error}"));
    let result = sandbox.read_text("../ava_onion_outside.txt").await;
    assert_eq!(result, Ok("secret".to_string()));
    // 系统临时目录可写（writableRoots 的 tmpdir 分支）。
    let scratch = std::env::temp_dir().join("ava_onion_scratch.txt");
    let temp_write = sandbox.write("../ava_onion_scratch.txt", "scratch").await;
    assert!(temp_write.is_ok(), "temp write must be allowed: {temp_write:?}");
    assert!(scratch.is_file(), "temp write must land under system temp");
    // 可写根之外（既不在权限根内、也不在 temp 内）仍然拒绝。
    let forbidden = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("forbidden_write.txt");
    let write_result = sandbox.write(forbidden.to_str().unwrap(), "nope").await;
    let error = write_result.expect_err("write outside writable roots must fail");
    assert!(error.contains("outside the permission root"), "got: {error}");
    let _ = std::fs::remove_file(&outside);
    let _ = std::fs::remove_file(&scratch);
}

#[tokio::test]
async fn read_image_returns_data_url_block() {
    let (sandbox, _root) = sandbox("image");
    // 1x1 红色 PNG 的最小合法字节序（仅验证编码路径，不校验像素）。
    let png: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8,
        0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00, 0x00,
        0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(sandbox.root().join("dot.png"), png)
        .unwrap_or_else(|error| panic!("seed png failed: {error}"));
    let block = sandbox.read_image("dot.png").await;
    let Ok(block) = block else {
        panic!("read image failed: {block:?}");
    };
    match &block {
        ContentBlock::Image { image_url } => {
            assert!(image_url.starts_with("data:image/png;base64,"), "unexpected url: {image_url}");
        }
        other => panic!("expected image block, got {other:?}"),
    }
}

#[tokio::test]
async fn list_returns_sorted_entries() {
    let (sandbox, root) = sandbox("list");
    std::fs::write(root.join("b.txt"), b"2").unwrap_or_else(|error| panic!("seed b failed: {error}"));
    std::fs::write(root.join("a.txt"), b"1").unwrap_or_else(|error| panic!("seed a failed: {error}"));
    let listing = sandbox.list("").await;
    assert_eq!(listing, Ok("a.txt\nb.txt".to_string()));
}

#[tokio::test]
async fn sandbox_is_shareable_across_tools() {
    let (sandbox, root) = sandbox("share");
    let shared = Arc::new(sandbox);
    let writer = shared.clone();
    let written = tokio::spawn(async move { writer.write("shared.txt", "x").await }).await;
    assert!(written.is_ok());
    assert!(root.join("shared.txt").exists(), "shared sandbox write must land under root");
}
