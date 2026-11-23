//! 图像上下文预算 — 进入模型上下文的图片统一限幅（源头瘦身）。
//!
//! 对齐 ava `ava-harness/src/tools/image_budget.rs` 的两层方案（本模块为层1）：
//!
//! - 进上下文前预算：data URL 图片超 [`MAX_IMAGE_PIXELS`] 像素按长边
//!   [`MAX_IMAGE_SIDE`] 与像素预算缩放重编码；超 [`MAX_IMAGE_BYTES`] 字节
//!   压缩重编码；解码失败/损坏 → 文本占位（失败降级而非请求失败）；非
//!   data URL（公网 URL）原样放行。入口在 `read_image` 工具返回前。
//! - 历史数量封顶：[`evict_old_images`] 仅保留最近 [`MAX_CONTEXT_IMAGES`]
//!   张图片，更早的替换为占位文本（请求视图级，state 原件不动）。
//!
//! 估算口径（层2）见 `compaction::estimate::IMAGE_TOKEN_ESTIMATE`：预算后
//! 的单图固定估算，不再按 data URL 字符数折算——估算必须面向预算后的请求
//! 视图，而非 state 中的原始 data URL。
//!
//! 图像处理为 CPU 密集操作，异步调用一律走 [`budget_blocks_async`]（经
//! `spawn_blocking` 移出异步热路径）；同步版本供测试与一次性入口直接调用。
//! 与 ava 实现的差异（教学子集）：不保留 ICC/EXIF 元数据，不处理 File/Video
//! 块（onion 类型面无此两类），无请求体总量护栏。

use base64::Engine as _;
use image::DynamicImage;
use image::GenericImageView as _;
use image::ImageEncoder;

use crate::types::{AgentMessage, ContentBlock};

/// 进上下文图片的像素预算（≈ Qwen 28px tile ~1.5k tokens，Claude 1568 长边
/// 约定 ~1.6k tokens 同量级）。
pub const MAX_IMAGE_PIXELS: u64 = 1_150_000;

/// 原样通过的字节上限（超过即解码重编码压缩体积）。
pub const MAX_IMAGE_BYTES: usize = 1024 * 1024;

/// 重编码时长边上限（遵循 Claude 1568 长边约定）。
const MAX_IMAGE_SIDE: u32 = 1568;

/// 请求视图内保留的最近图片数量上限（更早的图片替换为文本占位）。
///
/// 本地网关对视觉模型逐个声明 `max_images`（Qwen3.8-27B=5、GLM-5.3-Flash=1），
/// 超限请求被网关整体 400 拒绝；历史保留 4 张为最宽松模型留出当前输入余量。
pub const MAX_CONTEXT_IMAGES: usize = 4;

/// 图片解码失败/损坏且无法处理时的占位（失败降级而非请求失败）。
const IMAGE_UNPROCESSABLE_PLACEHOLDER: &str =
    "[image omitted: too large or corrupt to process]";
/// 老图淘汰占位（历史上下文数量封顶）。
const EVICTED_IMAGE_PLACEHOLDER: &str = "[image removed from earlier context to save tokens]";

/// 对内容块执行上下文预算（仅模型视图；state 原件不动）：
///
/// - Image：data URL 超预算降采样重编码；解码失败/损坏 → 文本占位；非
///   data URL（公网 URL）原样保留；
/// - ToolResult：内嵌图片逐张预算，解码失败的原位替换为文本占位；
/// - 其余块原样保留。
pub fn budget_blocks(blocks: Vec<ContentBlock>) -> Vec<ContentBlock> {
    blocks.into_iter().map(budget_block).collect()
}

/// [`budget_blocks`] 的异步版：图像解码/缩放/重编码为 CPU 密集操作（常见
/// 1~4MB 截图 <100ms），经 `spawn_blocking` 移出异步热路径，避免阻塞运行时
/// worker。无图像载荷时直接同步返回，省去任务调度开销。
pub async fn budget_blocks_async(blocks: Vec<ContentBlock>) -> Vec<ContentBlock> {
    if !has_image_payload(&blocks) {
        return blocks;
    }
    match tokio::task::spawn_blocking(move || budget_blocks(blocks)).await {
        Ok(budgeted) => budgeted,
        Err(error) => panic!("image budget blocking task failed: {error}"),
    }
}

/// 判断块列表是否含需要图像解码预算的 data URL 图片（顶层或工具结果内嵌）。
fn has_image_payload(blocks: &[ContentBlock]) -> bool {
    blocks.iter().any(|b| match b {
        ContentBlock::Image { image_url } => is_data_url(image_url),
        ContentBlock::ToolResult { content, .. } => content
            .iter()
            .any(|b| matches!(b, ContentBlock::Image { image_url } if is_data_url(image_url))),
        _ => false,
    })
}

fn budget_block(block: ContentBlock) -> ContentBlock {
    match block {
        ContentBlock::Image { image_url } => match budget_data_url_checked(&image_url) {
            Ok(url) => ContentBlock::Image { image_url: url },
            Err(placeholder) => ContentBlock::Text {
                text: placeholder.to_string(),
            },
        },
        ContentBlock::ToolResult { tool_call_id, is_error, content } => ContentBlock::ToolResult {
            tool_call_id,
            is_error,
            content: content.into_iter().map(budget_block).collect(),
        },
        other => other,
    }
}

/// 对 data URL 图片执行上下文预算：超预算则降采样重编码；解码失败/损坏返回
/// `Err(占位文案)`（由调用方把块替换为文本占位）；非 data URL（公网 URL /
/// 路径引用）原样放行。
///
/// 同步 CPU 操作（解码/缩放/编码）。
pub fn budget_data_url_checked(url: &str) -> Result<String, &'static str> {
    let bytes = match parse_data_url(url) {
        DataUrlParse::NotDataUrl => return Ok(url.to_string()),
        DataUrlParse::Invalid => return Err(IMAGE_UNPROCESSABLE_PLACEHOLDER),
        DataUrlParse::Valid { bytes } => bytes,
    };
    // 快路径：字节与像素（头部嗅探）均在预算内 → 原样通过，不解码。
    let within_pixels = header_pixel_count(&bytes).is_none_or(|px| px <= MAX_IMAGE_PIXELS);
    if bytes.len() <= MAX_IMAGE_BYTES && within_pixels {
        return Ok(url.to_string());
    }
    // 超预算：解码 → 缩放/重编码；无法缩小时保留原图（失败降级）。
    let Some(decoded) = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.decode().ok())
    else {
        return Err(IMAGE_UNPROCESSABLE_PLACEHOLDER);
    };
    let scaled = scale_to_budget(&decoded);
    match encode_shrunk(&scaled, bytes.len()) {
        Some(out) => Ok(out),
        None => Ok(url.to_string()),
    }
}

/// 请求视图内老图淘汰：仅保留最近 [`MAX_CONTEXT_IMAGES`] 张图片（顶层 +
/// 工具结果内嵌统一计数，按消息顺序），更早的替换为文本占位。state 原始
/// 消息不动，仅作用于发往模型的请求视图。
pub fn evict_old_images(messages: &[AgentMessage]) -> Vec<AgentMessage> {
    let counts: Vec<usize> = messages.iter().map(|m| count_images(&m.blocks)).collect();
    let total: usize = counts.iter().sum();
    if total <= MAX_CONTEXT_IMAGES {
        return messages.to_vec();
    }
    // 顺序遍历：每条消息需淘汰数 = max(0, 该消息起剩余图片数 - 保留配额)。
    let mut remaining = total;
    messages
        .iter()
        .zip(counts)
        .map(|(m, count)| {
            let evict_here = remaining.saturating_sub(MAX_CONTEXT_IMAGES);
            remaining = remaining.saturating_sub(count);
            if evict_here == 0 {
                return m.clone();
            }
            let mut to_evict = evict_here;
            let blocks = m
                .blocks
                .iter()
                .map(|b| evict_in_block(b, &mut to_evict))
                .collect();
            AgentMessage {
                role: m.role.clone(),
                blocks,
                source: m.source.clone(),
            }
        })
        .collect()
}

/// 统计块列表中的图片数（顶层 Image + ToolResult 内嵌），作为淘汰配额的
/// 计数单位。
fn count_images(blocks: &[ContentBlock]) -> usize {
    blocks
        .iter()
        .map(|b| match b {
            ContentBlock::Image { .. } => 1,
            ContentBlock::ToolResult { content, .. } => content
                .iter()
                .filter(|b| matches!(b, ContentBlock::Image { .. }))
                .count(),
            _ => 0,
        })
        .sum()
}

/// 单块淘汰重放：按 `to_evict` 配额把最靠前的图片替换为占位文本。
fn evict_in_block(block: &ContentBlock, to_evict: &mut usize) -> ContentBlock {
    match block {
        ContentBlock::Image { .. } if *to_evict > 0 => {
            *to_evict -= 1;
            ContentBlock::Text {
                text: EVICTED_IMAGE_PLACEHOLDER.into(),
            }
        }
        ContentBlock::ToolResult { tool_call_id, is_error, content } => ContentBlock::ToolResult {
            tool_call_id: tool_call_id.clone(),
            is_error: *is_error,
            content: content.iter().map(|b| evict_in_block(b, to_evict)).collect(),
        },
        other => other.clone(),
    }
}

/// 缩放到预算内：仅当超出长边 [`MAX_IMAGE_SIDE`] 时缩略（`thumbnail` 对小于
/// 目标框的图会按框上采样放大，必须显式跳过），仍超像素预算再按面积比例缩，
/// 缩放尺寸向下取整（floor 对齐）确保整数尺寸严格落在预算内。
fn scale_to_budget(img: &DynamicImage) -> DynamicImage {
    let (w0, h0) = img.dimensions();
    let scaled = if w0 > MAX_IMAGE_SIDE || h0 > MAX_IMAGE_SIDE {
        img.thumbnail(MAX_IMAGE_SIDE, MAX_IMAGE_SIDE)
    } else {
        img.clone()
    };
    let (w, h) = scaled.dimensions();
    let px = u64::from(w) * u64::from(h);
    if px <= MAX_IMAGE_PIXELS {
        return scaled;
    }
    // 面积比例缩放：target_scale = sqrt(MAX_IMAGE_PIXELS / px)，floor 保证不超。
    let ratio = ((MAX_IMAGE_PIXELS as f64) / (px as f64)).sqrt();
    let tw = (((w as f64) * ratio).floor() as u32).max(1);
    let th = (((h as f64) * ratio).floor() as u32).max(1);
    scaled.resize_exact(tw, th, image::imageops::FilterType::Triangle)
}

/// 重编码并尽量小于原字节：含透明通道走 PNG（保 alpha），否则 JPEG q85
/// （`encode_image` 自行处理 alpha 拍平）；两轮均未缩小时返回 `None`
/// （调用方原样保留原图，失败降级）。
fn encode_shrunk(img: &DynamicImage, original_len: usize) -> Option<String> {
    if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        let encoded_ok = encoder
            .write_image(rgba.as_raw(), img.width(), img.height(), image::ColorType::Rgba8.into())
            .is_ok();
        if encoded_ok && buf.len() < original_len {
            return Some(to_data_url("image/png", &buf));
        }
    }
    let mut buf = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 85);
    encoder.encode_image(img).ok()?;
    (buf.len() < original_len).then(|| to_data_url("image/jpeg", &buf))
}

/// 字节封装为 base64 data URL。
fn to_data_url(mime: &str, bytes: &[u8]) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// 判断是否为 data URL（`data:` 前缀；公网 URL / 路径引用为廉价载荷不参与处理）。
fn is_data_url(s: &str) -> bool {
    s.starts_with("data:")
}

/// data URL 解析分类。
enum DataUrlParse {
    /// 非 base64 data URL（公网 URL / 路径引用 / 非 base64 data URL）：原样放行。
    NotDataUrl,
    /// base64 载荷损坏：占位替换（不再原样透传坏图）。
    Invalid,
    /// 解析成功。
    Valid { bytes: Vec<u8> },
}

/// 解析 data URL；非 base64 data URL 视为非 data URL 载荷放行。
fn parse_data_url(url: &str) -> DataUrlParse {
    let Some(rest) = url.strip_prefix("data:") else {
        return DataUrlParse::NotDataUrl;
    };
    let Some((_, payload)) = rest.split_once(";base64,") else {
        return DataUrlParse::NotDataUrl;
    };
    match base64::engine::general_purpose::STANDARD.decode(payload.trim()) {
        Ok(bytes) => DataUrlParse::Valid { bytes },
        Err(_) => DataUrlParse::Invalid,
    }
}

/// 从图片头部嗅探像素总数（宽×高），无法识别返回 `None`。
///
/// 纯头部解析（不解码像素）：PNG IHDR / JPEG SOF / GIF / BMP / WebP-VP8X。
fn header_pixel_count(bytes: &[u8]) -> Option<u64> {
    let (w, h) = header_dimensions(bytes)?;
    Some(u64::from(w) * u64::from(h))
}

/// 从图片头部嗅探 `(宽, 高)`，无法识别返回 `None`。
fn header_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() >= 24 && bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        // IHDR：宽高固定位于 16..24（BE u32）。
        let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        return Some((w, h));
    }
    if bytes.len() >= 10 && bytes.starts_with(b"GIF8") {
        let w = u16::from_le_bytes(bytes[6..8].try_into().ok()?) as u32;
        let h = u16::from_le_bytes(bytes[8..10].try_into().ok()?) as u32;
        return Some((w, h));
    }
    if bytes.len() >= 26 && bytes.starts_with(b"BM") {
        let w = i32::from_le_bytes(bytes[18..22].try_into().ok()?).unsigned_abs();
        let h = i32::from_le_bytes(bytes[22..26].try_into().ok()?).unsigned_abs();
        return Some((w, h));
    }
    if bytes.len() >= 30 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return webp_dimensions(bytes);
    }
    if bytes.len() >= 10 && bytes.starts_with(&[0xFF, 0xD8]) {
        return jpeg_dimensions(bytes);
    }
    None
}

/// WebP 尺寸嗅探：VP8X（扩展头）从 canvas 字段取；VP8/VP8L 返回 `None`
/// （快路径仅用于预算预判，无法识别时仅走字节门槛，超限仍会完整解码）。
fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if &bytes[12..16] == b"VP8X" {
        // VP8X: 24..26 宽-1（24bit LE），27..29 高-1（24bit LE）。
        let read24 =
            |off: usize| u32::from(bytes[off]) | u32::from(bytes[off + 1]) << 8 | u32::from(bytes[off + 2]) << 16;
        return Some((read24(24) + 1, read24(27) + 1));
    }
    None
}

/// JPEG 尺寸嗅探：扫描 SOFn 段（0xC0..=0xCF，排除 C4/C8/CC）。
fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2usize;
    while i + 9 <= bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        let is_sof = (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker);
        if is_sof {
            let h = u16::from_be_bytes(bytes[i + 5..i + 7].try_into().ok()?) as u32;
            let w = u16::from_be_bytes(bytes[i + 7..i + 9].try_into().ok()?) as u32;
            return Some((w, h));
        }
        // 跳过段：段长 2 字节 BE（含自身两字节）。
        let len = usize::from(u16::from_be_bytes(bytes[i + 2..i + 4].try_into().ok()?));
        i += 2 + len;
    }
    None
}

#[cfg(test)]
#[path = "media_tests.rs"]
mod tests;
