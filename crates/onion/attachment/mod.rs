//! 附件体系：durable 内容寻址存储 + 准入。
//!
//! 语义分层：types/admission/error 为抽象 seam，本地文件实现同模块承载：
//!
//! - **内容寻址存储**（[`store`]）：sha256 摘要为 id（`sha256:<hex>`），
//!   staged 写（tmp → fsync → hard-link → EEXIST 复核 → 只读权限），
//!   对象去重；图片存 `objects/<2hex>/<hex>`，文件存
//!   `file-objects/<2hex>/<hex>`（canonical）+ `files/<2hex>/<hex>/<叶名>`（别名）。
//! - **准入**（[`admission`]）：canonical base64 复核、批量限额
//!   （张数 / 聚合字节 / 单图字节 / 像素 / 边长 / 媒体类型），prepare-then-commit
//!   全量校验后才落盘（一批一张非法则整批拒绝、零对象写入）。
//! - **稳定错误码**（[`AttachmentError`]）：稳定路由键
//!   （`INVALID_IMAGE_BASE64` / `IMAGE_TYPE_MISMATCH` / `ATTACHMENT_CORRUPT` 等），
//!   宿主按码映射 HTTP 状态与文案。
//!
//! 设计取舍：字节**原样入库**（不做入库前重编码归一），请求侧的
//! 压缩/重编码/预算淘汰由 `crate::media`（data URL 预算管线）承担；EXIF 方向
//! 旋转与 sharp 变体缓存未携带。媒体嗅探走 `image` crate（guess format + 尺寸），
//! 覆盖 PNG/JPEG/GIF/WebP/BMP。

pub mod admission;
pub mod store;

use serde::{Deserialize, Serialize};

pub use admission::{admit_encoded_file, admit_encoded_images, ImageAttachmentLimits};
pub use store::{AttachmentStore, SaveFile, SaveImage};

/// 稳定错误码 + 文案。宿主按 [`Self::code`]
/// 路由；图片准入与 base64 类错误属调用方可纠正（HTTP 400），存储类为服务端故障。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentError {
    code: &'static str,
    message: String,
}

impl AttachmentError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    /// 稳定路由键。
    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// 图片准入 / base64 / 引用格式类错误：调用方可纠正，映射 HTTP 400。
    pub fn is_image_admission(&self) -> bool {
        matches!(
            self.code,
            "TOO_MANY_IMAGES"
                | "IMAGES_TOO_LARGE"
                | "UNSUPPORTED_IMAGE_TYPE"
                | "INVALID_IMAGE_BASE64"
                | "INVALID_IMAGE"
                | "IMAGE_TYPE_MISMATCH"
                | "IMAGE_TOO_LARGE"
                | "IMAGE_TOO_MANY_PIXELS"
                | "IMAGE_DIMENSION_TOO_LARGE"
                | "INVALID_FILE_BASE64"
                | "INVALID_ATTACHMENT_REF"
        )
    }
}

impl std::fmt::Display for AttachmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AttachmentError {}

/// 图片附件 durable 引用：`width` / `height` 为
/// 编码内蕴尺寸（未做 EXIF 方向旋转），供投影几何免读字节使用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAttachmentRef {
    pub attachment_id: String,
    pub media_type: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// 文件附件 durable 引用：字节原样存储。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileAttachmentRef {
    pub attachment_id: String,
    pub name: String,
    pub bytes: u64,
}

/// 待入库的编码图片：`data` 为 base64。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodedImageAttachment {
    pub media_type: String,
    pub data: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// 待入库的编码文件：允许空字节。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodedFileAttachment {
    pub data: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
