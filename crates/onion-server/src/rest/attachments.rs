//! `/v1/attachments`：durable 附件上传端点（内容寻址存储，见 [`onion::attachment`]）。
//!
//! - `POST /v1/attachments/images`：批量图片（canonical base64）→
//!   `ImageAttachmentRef`（张数/聚合字节/单图字节/像素/媒体类型限额，
//!   prepare-then-commit 原子性：一批一张非法则整批拒绝、零落盘）。
//! - `POST /v1/attachments/files`：单文件（canonical base64，允许空字节）→
//!   `FileAttachmentRef`（字节原样入库，别名目录挂净化叶名）。
//!
//! 图片进入模型上下文的压缩/重编码由 `onion::media` 在请求投影侧承担；
//! 本端点只负责 durable 存储与准入。存储根统一 `<data_home>/attachments/v1`。

use axum::Json;
use serde::Deserialize;

use onion::attachment::{
    admit_encoded_file, admit_encoded_images, AttachmentError, EncodedFileAttachment,
    FileAttachmentRef, ImageAttachmentLimits, ImageAttachmentRef,
};

use crate::rest::error::ApiError;

/// 附件存储根：`<data_home>/attachments/v1`（端点与消息入口共用同一根）。
pub(crate) fn attachment_store() -> onion::attachment::AttachmentStore {
    onion::attachment::AttachmentStore::new(onion::paths::attachments_dir(&onion::paths::data_home()))
}

/// 附件错误 → REST 错误映射：准入类（调用方可纠正）400 / 引用不存在 404 /
/// 其余存储失败 500；`code` 沿用稳定错误码。
pub(crate) fn map_attachment_error(error: AttachmentError) -> ApiError {
    if error.is_image_admission() {
        ApiError::bad_request(error.code(), error.message())
    } else if error.code() == "ATTACHMENT_NOT_FOUND" {
        ApiError::not_found(error.code(), error.message())
    } else {
        ApiError::internal(format!("{}: {}", error.code(), error.message()))
    }
}

/// `POST /v1/attachments/images` 请求体。
#[derive(Debug, Deserialize)]
pub(crate) struct UploadImagesRequest {
    pub images: Vec<onion::attachment::EncodedImageAttachment>,
}

/// `POST /v1/attachments/images` 响应体。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadImagesResponse {
    pub attachments: Vec<ImageAttachmentRef>,
}

/// `POST /v1/attachments/images`：批量准入 + 内容寻址发布（整批原子）。
pub(crate) async fn upload_images(
    Json(request): Json<UploadImagesRequest>,
) -> Result<Json<UploadImagesResponse>, ApiError> {
    let store = attachment_store();
    let attachments =
        admit_encoded_images(&store, &request.images, &ImageAttachmentLimits::default())
            .map_err(map_attachment_error)?;
    Ok(Json(UploadImagesResponse { attachments }))
}

/// `POST /v1/attachments/files`：单文件准入 + 发布（字节原样入库）。
pub(crate) async fn upload_file(
    Json(file): Json<EncodedFileAttachment>,
) -> Result<Json<FileAttachmentRef>, ApiError> {
    let store = attachment_store();
    let attachment = admit_encoded_file(&store, &file).map_err(map_attachment_error)?;
    Ok(Json(attachment))
}
