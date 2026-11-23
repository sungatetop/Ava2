//! `attachment::store` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::{detect_image, display_name, file_leaf_name, parse_attachment_id, sha256_hex, AttachmentStore, SaveFile, SaveImage};
use crate::attachment::{AttachmentError, FileAttachmentRef, ImageAttachmentRef};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ava_attachment_store_{}_{}_{}",
        name,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp root");
    dir
}

/// 清理测试根（只读权限文件须先复位权限，Windows 上 remove_dir_all 才可靠）。
fn cleanup(root: &Path) {
    fn clear_readonly(dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    clear_readonly(&path);
                } else {
                    clear_readonly_file(&path);
                }
            }
        }
    }
    clear_readonly(root);
    let _ = std::fs::remove_dir_all(root);
}

/// 复位单个文件的只读权限（测试内改写/删除已发布对象用）。
fn clear_readonly_file(path: &Path) {
    if let Ok(metadata) = std::fs::metadata(path) {
        let mut permissions = metadata.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o644);
        }
        #[cfg(windows)]
        permissions.set_readonly(false);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let canvas = image::RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(canvas)
        .write_to(&mut out, image::ImageFormat::Png)
        .expect("encode png");
    out.into_inner()
}

#[test]
fn save_image_publishes_deduped_object() {
    let root = temp_root("dedup");
    let store = AttachmentStore::new(&root);
    let data = png_bytes(2, 2);
    let input = SaveImage {
        data: data.clone(),
        media_type: "image/png".to_string(),
        name: Some("shots/截图 a.png".to_string()),
    };
    let first = store.save_image(input.clone()).expect("save image");
    let second = store.save_image(input).expect("save image again");
    assert_eq!(first, second);
    let hex = sha256_hex(&data);
    let object = root.join("objects").join(&hex[..2]).join(&hex);
    assert_eq!(std::fs::read(&object).expect("object bytes"), data);
    assert_eq!(
        std::fs::read_dir(object.parent().expect("bucket")).expect("bucket").count(),
        1
    );
    assert_eq!(first.attachment_id, format!("sha256:{hex}"));
    assert_eq!(first.media_type, "image/png");
    assert_eq!((first.width, first.height), (2, 2));
    assert_eq!(first.bytes, data.len() as u64);
    assert_eq!(first.name.as_deref(), Some("截图 a.png"));
    cleanup(&root);
}

#[test]
fn save_image_rejects_mismatched_declaration_and_undetectable() {
    let root = temp_root("reject");
    let store = AttachmentStore::new(&root);
    let mismatch = store
        .save_image(SaveImage {
            data: png_bytes(2, 2),
            media_type: "image/jpeg".to_string(),
            name: None,
        })
        .expect_err("mismatched declaration");
    assert_eq!(mismatch.code(), "IMAGE_TYPE_MISMATCH");
    let empty = store
        .save_image(SaveImage { data: Vec::new(), media_type: "image/png".to_string(), name: None })
        .expect_err("empty image");
    assert_eq!(empty.code(), "INVALID_IMAGE");
    let garbage = store
        .save_image(SaveImage {
            data: b"not an image at all".to_vec(),
            media_type: "image/png".to_string(),
            name: None,
        })
        .expect_err("undetectable bytes");
    assert_eq!(garbage.code(), "INVALID_IMAGE");
    cleanup(&root);
}

#[test]
fn read_image_verifies_digest_and_metadata() {
    let root = temp_root("verify");
    let store = AttachmentStore::new(&root);
    let data = png_bytes(3, 4);
    let reference = store
        .save_image(SaveImage { data: data.clone(), media_type: "image/png".to_string(), name: None })
        .expect("save");
    assert_eq!(store.read_image(&reference).expect("read"), data);
    // 摘要破坏：复位权限后改写对象字节 → ATTACHMENT_CORRUPT。
    let hex = sha256_hex(&data);
    let object = root.join("objects").join(&hex[..2]).join(&hex);
    clear_readonly_file(&object);
    std::fs::write(&object, b"corrupted").expect("overwrite");
    let damaged = store.read_image(&reference).expect_err("corrupt");
    assert_eq!(damaged.code(), "ATTACHMENT_CORRUPT");
    // 缺失：移走对象 → ATTACHMENT_NOT_FOUND。
    std::fs::remove_file(&object).expect("remove");
    let missing = store.read_image(&reference).expect_err("missing");
    assert_eq!(missing.code(), "ATTACHMENT_NOT_FOUND");
    cleanup(&root);
}

#[test]
fn read_rejects_malformed_attachment_id() {
    let root = temp_root("refid");
    let store = AttachmentStore::new(&root);
    let reference = ImageAttachmentRef {
        attachment_id: "../escape".to_string(),
        media_type: "image/png".to_string(),
        bytes: 1,
        width: 1,
        height: 1,
        name: None,
    };
    let error = store.read_image(&reference).expect_err("malformed id");
    assert_eq!(error.code(), "INVALID_ATTACHMENT_REF");
    assert!(parse_attachment_id("sha256:abc").is_err());
    assert!(parse_attachment_id("sha256:").is_err());
    assert!(parse_attachment_id("sha256:zz").is_err());
    cleanup(&root);
}

#[test]
fn save_file_stores_verbatim_with_alias_and_dedup() {
    let root = temp_root("files");
    let store = AttachmentStore::new(&root);
    let data = b"report body".to_vec();
    let first = store
        .save_file(SaveFile { data: data.clone(), name: Some("docs/报告 final.txt".to_string()) })
        .expect("save file");
    assert_eq!(first.name, "报告 final.txt");
    let hex = sha256_hex(&data);
    let object = root.join("file-objects").join(&hex[..2]).join(&hex);
    assert_eq!(std::fs::read(&object).expect("canonical bytes"), data);
    let alias = root.join("files").join(&hex[..2]).join(&hex).join("报告 final.txt");
    assert_eq!(std::fs::read(&alias).expect("alias bytes"), data);
    // 同内容不同名 → 同 id（canonical 去重），别名各自保留。
    let again = store
        .save_file(SaveFile { data, name: Some("alias2.txt".to_string()) })
        .expect("save again");
    assert_eq!(again.attachment_id, first.attachment_id);
    assert_eq!(again.name, "alias2.txt");
    let read_back = store.read_file(&first).expect("read file");
    assert_eq!(read_back, b"report body");
    // 空文件合法，叶名回退 file。
    let empty = store
        .save_file(SaveFile { data: Vec::new(), name: None })
        .expect("empty file");
    assert_eq!(empty.bytes, 0);
    assert_eq!(empty.name, "file");
    // 字节数声明不匹配 → CORRUPT；匹配则通过（读路径仅校验摘要 + 字节数）。
    let tampered = FileAttachmentRef { bytes: 999, ..first.clone() };
    let error = store.read_file(&tampered).expect_err("tampered");
    assert_eq!(error.code(), "ATTACHMENT_CORRUPT");
    let detached =
        FileAttachmentRef { attachment_id: first.attachment_id, bytes: 11, name: String::new() };
    assert!(store.read_file(&detached).is_ok());
    cleanup(&root);
}

#[test]
fn leaf_and_display_names_sanitize() {
    assert_eq!(file_leaf_name(Some("a/b/c.zip")), "c.zip");
    assert_eq!(file_leaf_name(Some("d\\e\\f.bin")), "f.bin");
    // `<>:` 三个连续非法字符逐个替换（无折叠）。
    assert_eq!(file_leaf_name(Some("bad<>:name?.txt")), "bad___name_.txt");
    // 控制字符删除而非替换。
    assert_eq!(file_leaf_name(Some("a\u{7f}b\u{1}c.txt")), "abc.txt");
    assert_eq!(file_leaf_name(Some("CON.txt")), "_CON.txt");
    // 设备名 stem 匹配前剥结尾点/空格。
    assert_eq!(file_leaf_name(Some("CON .txt")), "_CON .txt");
    assert_eq!(file_leaf_name(Some("  .  ")), "file");
    assert_eq!(file_leaf_name(None), "file");
    assert_eq!(display_name(Some("/x/y/图片 .png")), Some("图片 .png".to_string()));
    assert_eq!(display_name(Some("   ")), None);
    // 显示名按字符数截断（255 字符，非字节）。
    let long = "长".repeat(300);
    let capped = display_name(Some(&long)).expect("capped");
    assert_eq!(capped.chars().count(), 255);
    assert_eq!(capped, "长".repeat(255));
}

#[test]
fn detect_image_reports_media_type_and_dimensions() {
    let (media, width, height) = detect_image(&png_bytes(5, 6)).expect("png");
    assert_eq!((media.as_str(), width, height), ("image/png", 5, 6));
    assert!(detect_image(b"").is_err());
    assert!(detect_image(b"plain text").is_err());
}

#[test]
fn error_is_std_error_with_stable_code() {
    let error = AttachmentError::new("INVALID_IMAGE", "boom");
    assert_eq!(error.to_string(), "INVALID_IMAGE: boom");
    assert!(error.is_image_admission());
    let storage = AttachmentError::new("ATTACHMENT_CORRUPT", "boom");
    assert!(!storage.is_image_admission());
}
