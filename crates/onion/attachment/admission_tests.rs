//! `attachment::admission` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use base64::Engine;

use super::{admit_encoded_file, admit_encoded_images, ImageAttachmentLimits};
use crate::attachment::store::{sha256_hex, AttachmentStore};
use crate::attachment::{EncodedFileAttachment, EncodedImageAttachment};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ava_attachment_admission_{}_{}_{}",
        name,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp root");
    dir
}

fn cleanup(root: &Path) {
    let _ = std::fs::remove_dir_all(root);
}

fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let canvas = image::RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(canvas)
        .write_to(&mut out, image::ImageFormat::Png)
        .expect("encode png");
    out.into_inner()
}

fn encoded_png(width: u32, height: u32) -> EncodedImageAttachment {
    EncodedImageAttachment {
        media_type: "image/png".to_string(),
        data: base64::engine::general_purpose::STANDARD.encode(png_bytes(width, height)),
        name: None,
    }
}

#[test]
fn admit_encoded_images_roundtrips_and_dedups() {
    let root = temp_root("roundtrip");
    let store = AttachmentStore::new(&root);
    let image = encoded_png(2, 2);
    let refs =
        admit_encoded_images(&store, &[image.clone(), image], &ImageAttachmentLimits::default())
            .expect("admit");
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0], refs[1]);
    assert!(refs[0].attachment_id.starts_with("sha256:"));
    assert_eq!((refs[0].width, refs[0].height), (2, 2));
    // 两图同内容 → 仅一个对象。
    let hex = refs[0].attachment_id.trim_start_matches("sha256:").to_string();
    let bucket = root.join("objects").join(&hex[..2]);
    assert_eq!(std::fs::read_dir(&bucket).expect("bucket").count(), 1);
    cleanup(&root);
}

#[test]
fn admit_encoded_images_enforces_limits() {
    let store = AttachmentStore::new(temp_root("limits"));
    let one = encoded_png(2, 2);
    // 张数超限。
    let limits = ImageAttachmentLimits { max_images_per_message: 1, ..Default::default() };
    let error = admit_encoded_images(&store, &[one.clone(), one], &limits).expect_err("count");
    assert_eq!(error.code(), "TOO_MANY_IMAGES");
    // 单图字节超限。
    let limits = ImageAttachmentLimits { max_image_bytes: 8, ..Default::default() };
    let error = admit_encoded_images(&store, &[encoded_png(2, 2)], &limits).expect_err("bytes");
    assert_eq!(error.code(), "IMAGE_TOO_LARGE");
    // 媒体类型不在白名单（BMP 嗅探命中但默认白名单不含）。
    let canvas = image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]));
    let mut bmp = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(canvas)
        .write_to(&mut bmp, image::ImageFormat::Bmp)
        .expect("encode bmp");
    let bmp_input = EncodedImageAttachment {
        media_type: "image/bmp".to_string(),
        data: base64::engine::general_purpose::STANDARD.encode(bmp.into_inner()),
        name: None,
    };
    let error =
        admit_encoded_images(&store, &[bmp_input], &ImageAttachmentLimits::default()).expect_err("type");
    assert_eq!(error.code(), "UNSUPPORTED_IMAGE_TYPE");
    // 边长超限。
    let limits = ImageAttachmentLimits { max_image_dimension: 1, ..Default::default() };
    let error = admit_encoded_images(&store, &[encoded_png(2, 2)], &limits).expect_err("dimension");
    assert_eq!(error.code(), "IMAGE_DIMENSION_TOO_LARGE");
    // 聚合字节超限。
    let limits = ImageAttachmentLimits { max_message_image_bytes: 16, ..Default::default() };
    let error = admit_encoded_images(&store, &[encoded_png(2, 2)], &limits).expect_err("aggregate");
    assert_eq!(error.code(), "IMAGES_TOO_LARGE");
    cleanup(store.root());
}

#[test]
fn admit_encoded_images_is_atomic() {
    let root = temp_root("atomic");
    let store = AttachmentStore::new(&root);
    let good = encoded_png(2, 2);
    let bad = EncodedImageAttachment {
        media_type: "image/png".to_string(),
        data: "not base64!".to_string(),
        name: None,
    };
    let error = admit_encoded_images(&store, &[good, bad], &ImageAttachmentLimits::default())
        .expect_err("invalid batch");
    assert_eq!(error.code(), "INVALID_IMAGE_BASE64");
    // prepare-then-commit：整批拒绝 → objects 桶不存在（零对象写入）。
    assert!(!root.join("objects").exists());
    cleanup(&root);
}

#[test]
fn admit_rejects_non_canonical_and_whitespace_base64() {
    let root = temp_root("canonical");
    let store = AttachmentStore::new(&root);
    let image = EncodedImageAttachment {
        media_type: "image/png".to_string(),
        data: base64::engine::general_purpose::STANDARD.encode(png_bytes(1, 1))
            + "=" // 追加填充 → 非规范
        ,
        name: None,
    };
    let error = admit_encoded_images(&store, &[image], &ImageAttachmentLimits::default())
        .expect_err("non-canonical");
    assert_eq!(error.code(), "INVALID_IMAGE_BASE64");
    // 文件：缺填充同样拒绝。
    let file = EncodedFileAttachment { data: "aGVsbG8".to_string(), name: None };
    let error = admit_encoded_file(&store, &file).expect_err("missing padding");
    assert_eq!(error.code(), "INVALID_FILE_BASE64");
    cleanup(&root);
}

#[test]
fn admit_encoded_file_allows_empty_and_reports_digest() {
    let root = temp_root("emptyfile");
    let store = AttachmentStore::new(&root);
    let reference = admit_encoded_file(
        &store,
        &EncodedFileAttachment { data: String::new(), name: Some("empty.bin".to_string()) },
    )
    .expect("empty file");
    assert_eq!(reference.bytes, 0);
    assert_eq!(reference.name, "empty.bin");
    assert_eq!(
        reference.attachment_id,
        format!("sha256:{}", sha256_hex(&[]))
    );
    cleanup(&root);
}
