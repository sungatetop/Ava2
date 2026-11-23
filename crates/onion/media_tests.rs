//! `media` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use crate::types::{AgentMessage, ContentBlock, Role};

/// 生成 width×height 的 RGB 噪声 PNG 字节（噪声保证压缩后体积超字节预算）。
fn noise_png(width: u32, height: u32) -> Vec<u8> {
    let mut img = image::RgbImage::new(width, height);
    let mut seed: u64 = 0x243F_6A88_85A3_08D3;
    for pixel in img.pixels_mut() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *pixel = image::Rgb([seed as u8, (seed >> 8) as u8, (seed >> 16) as u8]);
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .expect("encode png");
    buf.into_inner()
}

/// 生成 width×height 的纯色 PNG 字节（体积小、像素多，用于像素预算路径）。
fn solid_png(width: u32, height: u32) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(width, height, image::Rgb([200, 40, 40]));
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .expect("encode png");
    buf.into_inner()
}

fn data_url(bytes: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// 解码 data URL 取 (宽, 高)（测试断言用）。
fn decoded_dims(url: &str) -> (u32, u32) {
    let payload = url.rsplit(";base64,").next().expect("base64 payload");
    let bytes = base64::engine::general_purpose::STANDARD.decode(payload).expect("decode");
    let img = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .expect("guess format")
        .decode()
        .expect("decode image");
    (img.width(), img.height())
}

fn user_image(url: &str) -> AgentMessage {
    AgentMessage {
        role: Role::User,
        blocks: vec![ContentBlock::Image { image_url: url.to_string() }],
        source: None,
    }
}

fn tool_image(url: &str) -> AgentMessage {
    AgentMessage {
        role: Role::Tool,
        blocks: vec![ContentBlock::ToolResult {
            tool_call_id: "call-1".into(),
            is_error: false,
            content: vec![ContentBlock::Image { image_url: url.to_string() }],
        }],
        source: None,
    }
}

// ---- budget_data_url_checked：像素/字节预算 ----

#[test]
fn small_image_passes_through_unchanged() {
    // 像素与字节均在预算内 → 原样通过（不解码，URL 字符串不变）。
    let url = data_url(&solid_png(64, 64));
    assert_eq!(budget_data_url_checked(&url).expect("预算内放行"), url);
}

#[test]
fn non_data_url_passthrough() {
    let url = "https://example.com/picture.png";
    assert_eq!(budget_data_url_checked(url).expect("公网 URL 放行"), url);
}

#[test]
fn corrupt_data_url_rejected_with_placeholder() {
    let error = budget_data_url_checked("data:image/png;base64,@@@not-base64@@@").unwrap_err();
    assert_eq!(error, "[image omitted: too large or corrupt to process]");
}

#[test]
fn oversized_pixels_scaled_to_budget() {
    // 1920×1920 = 3.69M px > 1.15M：解码 → 长边 1568 → 面积比例 → 预算内。
    let url = data_url(&noise_png(1920, 1920));
    let out = budget_data_url_checked(&url).expect("超像素预算重编码");
    assert_ne!(out, url, "应输出新 data URL");
    let (w, h) = decoded_dims(&out);
    assert!(
        u64::from(w) <= u64::from(MAX_IMAGE_SIDE) && u64::from(h) <= u64::from(MAX_IMAGE_SIDE),
        "长边 ≤ 1568：{w}×{h}"
    );
    assert!(
        u64::from(w) * u64::from(h) <= MAX_IMAGE_PIXELS,
        "像素预算内：{w}×{h} = {}",
        u64::from(w) * u64::from(h)
    );
    assert!(out.len() < url.len(), "重编码后体积应小于原图");
}

#[test]
fn oversized_bytes_recompressed_with_same_dims() {
    // 1000×1000 = 1.0M px 预算内，但噪声 PNG > 1MB：仅压缩重编码，尺寸不变。
    let url = data_url(&noise_png(1000, 1000));
    assert!(url.len() > MAX_IMAGE_BYTES, "前置：原图超字节预算");
    let out = budget_data_url_checked(&url).expect("超字节预算重编码");
    assert_ne!(out, url);
    assert_eq!(decoded_dims(&out), (1000, 1000), "像素在预算内不缩放");
    assert!(out.len() < url.len(), "JPEG 重编码后体积应小于原图");
}

#[test]
fn solid_large_image_scales_even_when_bytes_small() {
    // 纯色大图字节极小但像素超预算：快路径字节门槛放行不了（像素嗅探拦截）。
    let url = data_url(&solid_png(1920, 1920));
    let out = budget_data_url_checked(&url).expect("超像素预算重编码");
    assert_ne!(out, url);
    let (w, h) = decoded_dims(&out);
    assert!(u64::from(w) * u64::from(h) <= MAX_IMAGE_PIXELS);
}

// ---- budget_blocks：块级预算与占位降级 ----

#[test]
fn budget_blocks_replaces_unprocessable_image_with_text() {
    let blocks = vec![
        ContentBlock::Image { image_url: "data:image/png;base64,@@@".into() },
        ContentBlock::Text { text: "keep".into() },
    ];
    let out = budget_blocks(blocks);
    assert_eq!(
        out,
        vec![
            ContentBlock::Text {
                text: "[image omitted: too large or corrupt to process]".into()
            },
            ContentBlock::Text { text: "keep".into() },
        ]
    );
}

#[test]
fn budget_blocks_budgets_images_inside_tool_result() {
    let url = data_url(&solid_png(1920, 1920));
    let blocks = vec![ContentBlock::ToolResult {
        tool_call_id: "call-1".into(),
        is_error: false,
        content: vec![ContentBlock::Image { image_url: url }, ContentBlock::Text { text: "note".into() }],
    }];
    let out = budget_blocks(blocks);
    let ContentBlock::ToolResult { content, .. } = &out[0] else {
        panic!("工具结果块应保留");
    };
    assert_eq!(content.len(), 2, "块数不变");
    assert!(
        matches!(&content[0], ContentBlock::Image { image_url } if decoded_dims(image_url).0 <= MAX_IMAGE_SIDE),
        "内嵌图片应已预算"
    );
}

#[tokio::test]
async fn budget_blocks_async_preserves_plain_blocks() {
    let blocks = vec![ContentBlock::Text { text: "hello".into() }];
    let out = budget_blocks_async(blocks).await;
    assert_eq!(out, vec![ContentBlock::Text { text: "hello".into() }]);
}

#[tokio::test]
async fn budget_blocks_async_budgets_image() {
    let url = data_url(&solid_png(1920, 1920));
    let blocks = vec![ContentBlock::Image { image_url: url }];
    let out = budget_blocks_async(blocks).await;
    assert!(matches!(&out[0], ContentBlock::Image { image_url } if decoded_dims(image_url).0 <= MAX_IMAGE_SIDE));
}

// ---- evict_old_images：历史图片数量封顶 ----

#[test]
fn evict_noop_when_within_quota() {
    let messages: Vec<AgentMessage> = (0..MAX_CONTEXT_IMAGES)
        .map(|i| user_image(&format!("data:image/png;base64,AAAA{i}")))
        .collect();
    let view = evict_old_images(&messages);
    assert_eq!(view, messages);
}

#[test]
fn evict_oldest_top_level_images_first() {
    let messages: Vec<AgentMessage> = (0..6)
        .map(|i| user_image(&format!("data:image/png;base64,AAAA{i}")))
        .collect();
    let view = evict_old_images(&messages);
    for (idx, message) in view.iter().enumerate() {
        let is_placeholder = matches!(
            message.blocks.first(),
            Some(ContentBlock::Text { text }) if text.contains("removed from earlier context")
        );
        if idx < 2 {
            assert!(is_placeholder, "最老两张应被淘汰：idx {idx}");
        } else {
            assert_eq!(message.blocks.first(), messages[idx].blocks.first(), "最近四张保留：idx {idx}");
        }
    }
}

#[test]
fn evict_counts_and_replaces_tool_result_images() {
    let messages = vec![
        tool_image("data:image/png;base64,AAAA0"),
        tool_image("data:image/png;base64,AAAA1"),
        tool_image("data:image/png;base64,AAAA2"),
        tool_image("data:image/png;base64,AAAA3"),
        tool_image("data:image/png;base64,AAAA4"),
    ];
    let view = evict_old_images(&messages);
    let ContentBlock::ToolResult { content, .. } = &view[0].blocks[0] else {
        panic!("工具结果块应保留");
    };
    assert!(
        matches!(&content[0], ContentBlock::Text { text } if text.contains("removed from earlier context")),
        "工具结果内嵌最老图片应被淘汰"
    );
    let ContentBlock::ToolResult { content, .. } = &view[4].blocks[0] else {
        panic!("工具结果块应保留");
    };
    assert!(matches!(&content[0], ContentBlock::Image { .. }), "最近一张保留");
}

#[test]
fn evict_keeps_non_image_blocks_intact() {
    let url = "data:image/png;base64,AAAA";
    let messages = vec![
        AgentMessage {
            role: Role::Tool,
            blocks: vec![ContentBlock::ToolResult {
                tool_call_id: "call-0".into(),
                is_error: true,
                content: vec![
                    ContentBlock::Text { text: "before".into() },
                    ContentBlock::Image { image_url: url.into() },
                    ContentBlock::Text { text: "after".into() },
                ],
            }],
            source: None,
        },
        user_image(url),
        user_image(url),
        user_image(url),
        user_image(url),
    ];
    let view = evict_old_images(&messages);
    let ContentBlock::ToolResult { tool_call_id, is_error, content } = &view[0].blocks[0] else {
        panic!("工具结果块应保留");
    };
    assert_eq!(tool_call_id, "call-0", "配对字段不动");
    assert!(*is_error);
    assert_eq!(content.len(), 3, "块数不变，图片原位替换");
    assert_eq!(content[0], ContentBlock::Text { text: "before".into() });
    assert!(matches!(&content[1], ContentBlock::Text { .. }));
    assert_eq!(content[2], ContentBlock::Text { text: "after".into() });
}

// ---- header 嗅探 ----

#[test]
fn header_pixel_count_sniffs_png() {
    let bytes = solid_png(640, 480);
    assert_eq!(header_pixel_count(&bytes), Some(640 * 480));
}

#[test]
fn header_pixel_count_rejects_garbage() {
    assert_eq!(header_pixel_count(b"not an image at all........"), None);
}
