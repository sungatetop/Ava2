//! dto 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::path::Path;

use super::{parts_to_agent_message, ContentPart, FilePart};
use onion::attachment::AttachmentStore;
use onion::types::ContentBlock;

/// 1x1 PNG（70 字节，头签名 89504e470d0a1a0a）。
const PNG_1PX_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/// 测试用独立附件存储根（按标签隔离，进程内互不冲突）。
fn temp_store(tag: &str) -> AttachmentStore {
    let root = std::env::temp_dir().join(format!("onion_dto_{}_{tag}", std::process::id()));
    AttachmentStore::new(root)
}

/// 清理测试根（先递归复位只读位：Windows 上 `remove_dir_all` 拒绝只读文件）。
fn cleanup(root: &Path) {
    if !root.exists() {
        return;
    }
    clear_readonly(root);
    let _ = std::fs::remove_dir_all(root);
}

fn clear_readonly(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            clear_readonly(&path);
        } else if let Ok(metadata) = std::fs::metadata(&path) {
            let mut permissions = metadata.permissions();
            permissions.set_readonly(false);
            let _ = std::fs::set_permissions(&path, permissions);
        }
    }
}

fn file_part(name: &str, mime: &str, uri: Option<String>, bytes: Option<String>) -> ContentPart {
    ContentPart::File {
        file: FilePart { name: Some(name.into()), mime_type: Some(mime.into()), uri, bytes },
    }
}

/// bytes 图片：准入成功 → data URL `Image` 块（媒体类型来自存储字节复核），
/// 对象落盘且去重。
#[test]
fn file_part_bytes_image_admitted_as_data_url_block() {
    let store = temp_store("image");
    let parts = vec![file_part("pixel.png", "image/png", None, Some(PNG_1PX_B64.into()))];
    let message = parts_to_agent_message(&parts, &store).expect("admission passes");
    assert_eq!(message.blocks.len(), 1);
    let ContentBlock::Image { image_url } = &message.blocks[0] else {
        panic!("expected image block");
    };
    assert!(
        image_url.starts_with("data:image/png;base64,"),
        "unexpected image url prefix: {image_url}"
    );
    // 对象已落盘：objects 桶恰有一个内容寻址子目录。
    let buckets: Vec<_> =
        std::fs::read_dir(store.root().join("objects")).expect("objects bucket").collect();
    assert_eq!(buckets.len(), 1);
    cleanup(store.root());
}

/// bytes 非图片文件：原样入库，以文本块描述引用（名称 / id / 字节数）。
#[test]
fn file_part_bytes_non_image_stored_verbatim_as_text_block() {
    let store = temp_store("file");
    let parts = vec![file_part("notes.txt", "text/plain", None, Some("aGVsbG8=".into()))];
    let message = parts_to_agent_message(&parts, &store).expect("admission passes");
    let ContentBlock::Text { text } = &message.blocks[0] else {
        panic!("expected text block");
    };
    assert!(
        text.starts_with("[file attachment stored: name=notes.txt id=sha256:") && text.ends_with("bytes=5]"),
        "unexpected description: {text}"
    );
    assert!(store.root().join("file-objects").exists());
    cleanup(store.root());
}

/// bytes 缺省：维持原行为（`uri` 透传为 `Image` 块，零落盘）。
#[test]
fn file_part_without_bytes_passes_uri_through() {
    let store = temp_store("uri");
    let parts = vec![file_part(
        "remote.png",
        "image/png",
        Some("https://example.com/a.png".into()),
        None,
    )];
    let message = parts_to_agent_message(&parts, &store).expect("passthrough");
    let ContentBlock::Image { image_url } = &message.blocks[0] else {
        panic!("expected image block");
    };
    assert_eq!(image_url, "https://example.com/a.png");
    assert!(!store.root().join("objects").exists());
    cleanup(store.root());
}

/// 非 canonical base64：整请求 400（INVALID_IMAGE_BASE64），零落盘。
#[test]
fn file_part_bytes_invalid_base64_rejected() {
    let store = temp_store("badb64");
    let parts = vec![file_part("a.png", "image/png", None, Some("not-base64!!!".into()))];
    let error = parts_to_agent_message(&parts, &store).expect_err("rejected");
    assert_eq!(error.code(), "INVALID_IMAGE_BASE64");
    assert!(!store.root().join("objects").exists());
    cleanup(store.root());
}

/// 批量张数限额：9 张超过缺省 8 张上限，整批拒绝且零对象落盘（原子性）。
#[test]
fn file_part_bytes_batch_limit_enforced_atomically() {
    let store = temp_store("batch");
    let parts: Vec<ContentPart> = (0..9)
        .map(|i| file_part(&format!("a{i}.png"), "image/png", None, Some(PNG_1PX_B64.into())))
        .collect();
    let error = parts_to_agent_message(&parts, &store).expect_err("rejected");
    assert_eq!(error.code(), "TOO_MANY_IMAGES");
    assert!(!store.root().join("objects").exists(), "整批拒绝必须零落盘");
    cleanup(store.root());
}

/// 混合 parts：text / image(bytes) / text 块序保持，图片占位回填正确。
#[test]
fn mixed_parts_preserve_block_order() {
    let store = temp_store("order");
    let parts = vec![
        ContentPart::Text { text: "before".into() },
        file_part("pixel.png", "image/png", None, Some(PNG_1PX_B64.into())),
        ContentPart::Text { text: "after".into() },
    ];
    let message = parts_to_agent_message(&parts, &store).expect("admission passes");
    assert_eq!(message.blocks.len(), 3);
    assert!(matches!(&message.blocks[0], ContentBlock::Text { text } if text == "before"));
    assert!(matches!(&message.blocks[1], ContentBlock::Image { .. }));
    assert!(matches!(&message.blocks[2], ContentBlock::Text { text } if text == "after"));
    cleanup(store.root());
}
