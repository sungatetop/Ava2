//! 附件准入：canonical base64 复核 +
//! 批量限额 + prepare-then-commit（全量校验通过才逐图入库，零部分写入）。

use base64::Engine;

use super::store::{AttachmentStore, SaveFile, SaveImage};
use super::{AttachmentError, EncodedFileAttachment, EncodedImageAttachment, FileAttachmentRef, ImageAttachmentRef};

/// 图片准入限额（各部署可自行解析；此处为缺省值，宿主可按需覆盖字段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAttachmentLimits {
    /// 单图解码后字节上限。
    pub max_image_bytes: usize,
    /// 单条消息（一批）图片张数上限。
    pub max_images_per_message: usize,
    /// 一批聚合解码字节上限。
    pub max_message_image_bytes: usize,
    /// 单图像素总数上限（防解码炸弹）。
    pub max_image_pixels: u64,
    /// 单图边长（宽或高）上限。
    pub max_image_dimension: u32,
    /// 允许入库的媒体类型（嗅探结果须命中）。
    pub media_types: Vec<String>,
}

impl Default for ImageAttachmentLimits {
    fn default() -> Self {
        Self {
            max_image_bytes: 10 * 1024 * 1024,
            max_images_per_message: 8,
            max_message_image_bytes: 40 * 1024 * 1024,
            max_image_pixels: 100_000_000,
            max_image_dimension: 16_384,
            media_types: ["image/png", "image/jpeg", "image/webp", "image/gif"]
                .map(str::to_string)
                .to_vec(),
        }
    }
}

/// 一批编码图片 → durable 引用。解码与限额全部通过后才入库（prepare-then-commit）。
pub fn admit_encoded_images(
    store: &AttachmentStore,
    images: &[EncodedImageAttachment],
    limits: &ImageAttachmentLimits,
) -> Result<Vec<ImageAttachmentRef>, AttachmentError> {
    let decoded = images
        .iter()
        .map(|image| decode_canonical(&image.data, "INVALID_IMAGE_BASE64", false))
        .collect::<Result<Vec<_>, _>>()?;
    if images.len() > limits.max_images_per_message {
        return Err(AttachmentError::new(
            "TOO_MANY_IMAGES",
            format!(
                "batch contains {} images, limit is {}",
                images.len(),
                limits.max_images_per_message
            ),
        ));
    }
    let total: usize = decoded.iter().map(Vec::len).sum();
    if total > limits.max_message_image_bytes {
        return Err(AttachmentError::new(
            "IMAGES_TOO_LARGE",
            format!(
                "batch aggregates {total} bytes, limit is {}",
                limits.max_message_image_bytes
            ),
        ));
    }
    // prepare：逐图限额校验 + 嗅探（媒体类型 / 尺寸 / 像素）。
    let mut prepared = Vec::with_capacity(decoded.len());
    for (input, data) in images.iter().zip(&decoded) {
        if data.len() > limits.max_image_bytes {
            return Err(AttachmentError::new(
                "IMAGE_TOO_LARGE",
                format!("image is {} bytes, limit is {}", data.len(), limits.max_image_bytes),
            ));
        }
        let (media_type, width, height) = super::store::detect_image(data)?;
        if !limits.media_types.iter().any(|value| value == &media_type) {
            return Err(AttachmentError::new(
                "UNSUPPORTED_IMAGE_TYPE",
                format!("media type {media_type} is not accepted"),
            ));
        }
        if input.media_type != media_type {
            return Err(AttachmentError::new(
                "IMAGE_TYPE_MISMATCH",
                format!(
                    "declared media type {} does not match detected {media_type}",
                    input.media_type
                ),
            ));
        }
        if width > limits.max_image_dimension || height > limits.max_image_dimension {
            return Err(AttachmentError::new(
                "IMAGE_DIMENSION_TOO_LARGE",
                format!("image dimensions {width}x{height} exceed {}", limits.max_image_dimension),
            ));
        }
        let pixels = u64::from(width) * u64::from(height);
        if pixels > limits.max_image_pixels {
            return Err(AttachmentError::new(
                "IMAGE_TOO_MANY_PIXELS",
                format!("image has {pixels} pixels, limit is {}", limits.max_image_pixels),
            ));
        }
        prepared.push(SaveImage {
            data: data.clone(),
            media_type,
            name: input.name.clone(),
        });
    }
    // commit：全部就绪后才落盘。
    prepared
        .into_iter()
        .map(|input| store.save_image(input))
        .collect()
}

/// 单个编码文件 → durable 引用（空字节合法）。
pub fn admit_encoded_file(
    store: &AttachmentStore,
    input: &EncodedFileAttachment,
) -> Result<FileAttachmentRef, AttachmentError> {
    let data = decode_canonical(&input.data, "INVALID_FILE_BASE64", true)?;
    store.save_file(SaveFile { data, name: input.name.clone() })
}

/// canonical base64 解码：严格校验，重编码必须逐字节相等。
/// 图片禁空，文件允许空。
fn decode_canonical(
    data: &str,
    code: &'static str,
    allow_empty: bool,
) -> Result<Vec<u8>, AttachmentError> {
    if data.is_empty() {
        if allow_empty {
            return Ok(Vec::new());
        }
        return Err(AttachmentError::new(
            code,
            "encoded payload must not be empty",
        ));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| AttachmentError::new(code, "payload is not valid base64"))?;
    if base64::engine::general_purpose::STANDARD.encode(&bytes) != data {
        return Err(AttachmentError::new(
            code,
            "payload is not canonical base64",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
