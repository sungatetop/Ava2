//! office 工具三件套：`office_convert` / `office_recalculate` / `office_render`。
//!
//! 共享 [`Shared`] = 沙箱 + soffice 引擎 + 附件库：
//! - 输入经沙箱受控原语 `stat` / `read_bytes` 读取，超上限报 `input-too-large`；
//! - 产物落**私有输出店**（系统临时目录，沙箱可写根内），工具只回绝对路径并提示
//!   模型经 shell 自行拷贝——不写会话工作区，保留期 24h 懒清扫；
//! - `office_render` 的首页 PNG 同时入附件库（复用 attachment 体系）并作为图片块
//!   进上下文（经 media 预算限幅）；
//! - 错误统一为 `code: message`（[`OfficeError::Display`]），注册表合成 is_error。
//!
//! 已知限制：render 仅渲染首页/首表/首屏
//! （CLI 无逐页渲染能力），完整文档请用 `office_convert` 转 pdf；
//! recalculate 经同格式 round-trip 实现。

use std::path::Path;
use std::sync::Arc;

use base64::Engine as _;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::ToolPort;
use crate::attachment::{AttachmentStore, SaveImage};
use crate::office::{self, Converted, Engine, Family, OfficeError};
use crate::sandbox::Sandbox;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// 三工具共享状态（装配期构造一次，Arc 分发给三个工具）。
pub struct Shared {
    pub sandbox: Arc<Sandbox>,
    /// `None` = 未定位到 soffice：工具照常注册，执行期报 `unavailable`。
    pub engine: Option<Engine>,
    pub attachments: AttachmentStore,
}

impl Shared {
    /// 生产装配：定位 soffice + 数据根附件库。
    pub fn new(sandbox: Arc<Sandbox>) -> Self {
        Self {
            sandbox,
            engine: Engine::locate(),
            attachments: AttachmentStore::new(crate::paths::attachments_dir(
                &crate::paths::data_home(),
            )),
        }
    }
}

/// 必填字符串参数（缺失 / 空串 / 非字符串 → `invalid-request`）。
fn require_string(call: &ToolCall, key: &str) -> Result<String, OfficeError> {
    call.input
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or(OfficeError::InvalidRequest)
}

/// 输入路径 → （小写扩展名, 家族）：扩展名缺失或不支持 → `unsupported-format`。
fn input_family(input: &str) -> Result<(String, Family), OfficeError> {
    let extension = Path::new(input)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or(OfficeError::UnsupportedFormat)?;
    let family = office::family_of(&extension).ok_or(OfficeError::UnsupportedFormat)?;
    Ok((extension, family))
}

/// 沙箱受控读取源文件（先 stat 校验上限，再读字节）。
async fn read_source(shared: &Shared, input: &str) -> Result<Vec<u8>, OfficeError> {
    let metadata = shared
        .sandbox
        .stat(input)
        .await
        .map_err(OfficeError::Failed)?;
    if metadata.len() > office::MAX_INPUT_BYTES {
        return Err(OfficeError::InputTooLarge);
    }
    shared
        .sandbox
        .read_bytes(input)
        .await
        .map_err(OfficeError::Failed)
}

/// 引擎执行一次转换（引擎缺失 → `unavailable`）。
async fn execute(
    shared: &Shared,
    source: &[u8],
    extension: &str,
    target: &str,
) -> Result<Converted, OfficeError> {
    let engine = shared.engine.as_ref().ok_or(OfficeError::Unavailable)?;
    let run_dir = office::create_run_dir()?;
    engine.convert(source, extension, target, &run_dir).await
}

/// 转换成功的内容投影（模型侧：路径 + 拷贝提示）。
fn converted_block(input: &str, converted: &Converted) -> ContentBlock {
    ContentBlock::Text {
        text: format!(
            "converted {input} -> {} ({} bytes)\nfile: {}\nCopy the file to where you need it \
             with shell; the output store is cleaned up after 24h.",
            converted.format,
            converted.bytes,
            converted.path.display()
        ),
    }
}

/// `office_convert`：同家族格式转换（文本 / 表格 / 演示 → pdf 或同家族格式）。
pub struct ConvertTool {
    pub shared: Arc<Shared>,
}

impl ToolPort for ConvertTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "office_convert".to_string(),
            description: "Convert a document to another format with headless LibreOffice and \
                return the output file path. Text: doc/docx/odt/rtf/txt -> pdf/docx/odt/txt; \
                spreadsheets: xls/xlsx/ods/csv -> pdf/xlsx/ods/csv; presentations: \
                ppt/pptx/odp -> pdf/pptx/odp. Input: {\"input\": string, \"format\": string}."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "path to the source document" },
                    "format": { "type": "string", "description": "target format allowed for the input family" }
                },
                "required": ["input", "format"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let shared = self.shared.clone();
        Box::pin(async move { convert(&shared, &call).await.map_err(|error| error.to_string()) })
    }
}

async fn convert(shared: &Shared, call: &ToolCall) -> Result<Vec<ContentBlock>, OfficeError> {
    let input = require_string(call, "input")?;
    let target = require_string(call, "format")?.to_ascii_lowercase();
    let (extension, family) = input_family(&input)?;
    if !office::conversion_targets(family).contains(&target.as_str()) {
        return Err(OfficeError::UnsupportedFormat);
    }
    let source = read_source(shared, &input).await?;
    let converted = execute(shared, &source, &extension, &target).await?;
    Ok(vec![converted_block(&input, &converted)])
}

/// `office_recalculate`：工作簿重算（同格式 round-trip，LibreOffice 加载时重算公式）。
pub struct RecalculateTool {
    pub shared: Arc<Shared>,
}

impl RecalculateTool {
    fn schema() -> ToolDefinition {
        ToolDefinition {
            name: "office_recalculate".to_string(),
            description: "Recalculate all formulas in a spreadsheet by round-tripping it through \
                headless LibreOffice, preserving the workbook format. Input: {\"input\": string, \
                \"format\": \"xlsx\" | \"ods\"} (format must match the input file).".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "path to the workbook (xls/xlsx/ods/csv)" },
                    "format": { "type": "string", "enum": ["xlsx", "ods"], "description": "workbook format; must match the input extension" }
                },
                "required": ["input", "format"]
            }),
        }
    }
}

impl ToolPort for RecalculateTool {
    fn definition(&self) -> ToolDefinition {
        Self::schema()
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let shared = self.shared.clone();
        Box::pin(async move { recalculate(&shared, &call).await.map_err(|error| error.to_string()) })
    }
}

async fn recalculate(shared: &Shared, call: &ToolCall) -> Result<Vec<ContentBlock>, OfficeError> {
    let input = require_string(call, "input")?;
    let format = require_string(call, "format")?.to_ascii_lowercase();
    let (extension, family) = input_family(&input)?;
    if family != Family::Spreadsheet || format != extension {
        return Err(OfficeError::UnsupportedFormat);
    }
    let source = read_source(shared, &input).await?;
    let converted = execute(shared, &source, &extension, &format).await?;
    Ok(vec![converted_block(&input, &converted)])
}

/// `office_render`：首页渲染成 PNG（入附件库 + 多模态图片块）。
pub struct RenderTool {
    pub shared: Arc<Shared>,
}

impl ToolPort for RenderTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "office_render".to_string(),
            description: "Render the first page (or first sheet / slide) of a document or PDF to \
                PNG and return it as an image block. Use office_convert to pdf for the whole \
                document. Input: {\"input\": string}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "path to the document (text, spreadsheet, presentation or pdf)" }
                },
                "required": ["input"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let shared = self.shared.clone();
        Box::pin(async move { render(&shared, &call).await.map_err(|error| error.to_string()) })
    }
}

async fn render(shared: &Shared, call: &ToolCall) -> Result<Vec<ContentBlock>, OfficeError> {
    let input = require_string(call, "input")?;
    let (extension, _family) = input_family(&input)?;
    let source = read_source(shared, &input).await?;
    let converted = execute(shared, &source, &extension, "png").await?;
    let data = office::read_output(&converted).await?;
    let name = Path::new(&input)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| format!("{stem}.png"));
    let reference = shared
        .attachments
        .save_image(SaveImage { data: data.clone(), media_type: "image/png".to_string(), name })
        .map_err(|error| OfficeError::Failed(format!("{}: {}", error.code(), error.message())))?;
    let summary = ContentBlock::Text {
        text: format!(
            "rendered first page of {input} -> png ({} bytes)\nfile: {}\nattachment: {}\nOnly the \
             first page / sheet / slide is rendered; use office_convert to pdf for the full document.",
            converted.bytes,
            converted.path.display(),
            reference.attachment_id
        ),
    };
    let image_url =
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&data));
    Ok(crate::media::budget_blocks_async(vec![summary, ContentBlock::Image { image_url }]).await)
}
