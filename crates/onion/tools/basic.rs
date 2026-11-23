//! 沙箱文件工具：read / read_image / write（全部经 Sandbox 受控原语）。
//!
//! 工具实现契约：不直接触碰 `tokio::fs`；执行错误返回 `Err(String)`（注册表合成
//! is_error 结果）；`write` 覆写 `risk()` 为 WorkspaceWrite（审批由部署侧策略派生）。
//!
//! 命名沿用 `tool-fs`：`read`（文本）与 `read_image`（图片多模态）职责分离，
//! 模型按文件类型选型明确；沙箱层各自 fail-closed 拒绝错配。

use std::sync::Arc;

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::ToolPort;
use crate::approval::ToolRisk;
use crate::fs_observation::{key_of, version_of, FsObservation, Observed};
use crate::sandbox::Sandbox;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// `read_image` 的相对路径入参 schema（`file_path`）。
fn image_path_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "file_path": { "type": "string", "description": "relative file path under sandbox root" } },
        "required": ["file_path"]
    })
}

/// 读成功登记观测（观察即事实）。
fn record_present(observation: &FsObservation, path: &str, data: &[u8]) {
    observation.refresh(&key_of(std::path::Path::new(path)), version_of(data));
}

/// 读失败且目标确实不存在时登记缺失观察（缺失也是有效观察，允许后续创建）。
fn record_absent(observation: &FsObservation, path: &str) {
    observation.record(&key_of(std::path::Path::new(path)), Observed::Absent);
}

/// read：读取沙箱内文本文件（图片文件报错并指向 `read_image`）。
/// 读结果登记 read-before-write 观测（改写既有文件的前置门）。
pub struct ReadTool {
    pub sandbox: Arc<Sandbox>,
    pub observation: Arc<FsObservation>,
}

impl ToolPort for ReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read".to_string(),
            description: "Read a text file under the sandbox root as numbered lines. Use read_image \
                for image files. Input: {\"file_path\": string, \"offset\"?: number, \"limit\"?: number}."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "relative file path under sandbox root" },
                    "offset": { "type": "integer", "description": "1-based start line to read (default 1)" },
                    "limit": { "type": "integer", "description": "Max lines to return (default 2000)" }
                },
                "required": ["file_path"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let sandbox = self.sandbox.clone();
        let observation = self.observation.clone();
        Box::pin(async move {
            let path = call.input.get("file_path").and_then(Value::as_str).unwrap_or("");
            let offset = call.input.get("offset").and_then(Value::as_u64).map(|n| n as usize).unwrap_or(1);
            let limit = call.input.get("limit").and_then(Value::as_u64).map(|n| n as usize).unwrap_or(2000);
            match sandbox.read_text(path).await {
                Ok(text) => {
                    record_present(&observation, path, text.as_bytes());
                    Ok(vec![ContentBlock::Text { text: paginate(&text, offset, limit) }])
                }
                // 仅「文件确实不存在」记缺失观察；类型错配等错误不污染观测表。
                Err(error) => {
                    if sandbox.read_bytes(path).await.is_err() {
                        record_absent(&observation, path);
                    }
                    Err(error)
                }
            }
        })
    }
}

/// 对全文做行分页（offset 1-based，返回带行号文本 + 分页 footer）。
fn paginate(full: &str, offset: usize, limit: usize) -> String {
    let lines: Vec<&str> = full.lines().collect();
    let start = offset.saturating_sub(1).min(lines.len());
    let end = start.saturating_add(limit).min(lines.len());
    let mut out = String::new();
    for (index, line) in lines[start..end].iter().enumerate() {
        out.push_str(&format!("{:>4}\t{line}\n", start + index + 1));
    }
    if end < lines.len() {
        out.push_str(&format!(
            "\n(lines {}–{} of {}. Use offset={} to read the next page.)\n",
            start + 1,
            end,
            lines.len(),
            end + 1
        ));
    }
    out
}

/// read_image：读取沙箱内图片文件，输出 data URL 图片块（多模态）。
/// 读结果登记观测（图片也可被 str_replace_editor 观察）。
pub struct ReadImageTool {
    pub sandbox: Arc<Sandbox>,
    pub observation: Arc<FsObservation>,
}

impl ToolPort for ReadImageTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_image".to_string(),
            description: "Read an image file (png/jpg/jpeg/gif/webp) under the sandbox root and \
                return it as an image block. Use read for text files. Input: {\"file_path\": string}."
                .to_string(),
            parameters: image_path_schema(),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let sandbox = self.sandbox.clone();
        let observation = self.observation.clone();
        Box::pin(async move {
            let path = call.input.get("file_path").and_then(Value::as_str).unwrap_or("");
            match sandbox.read_image(path).await {
                Ok(block) => {
                    // 图片内容版本凭证同样以字节摘要登记（经沙箱原语，工具不触碰 fs）。
                    if let Ok(data) = sandbox.read_bytes(path).await {
                        record_present(&observation, path, &data);
                    }
                    // 层1 图片预算：进上下文前像素/字节限幅（超限图解码缩放
                    // 重编码；解码失败降级为占位文本），见 media 模块。
                    Ok(crate::media::budget_blocks_async(vec![block]).await)
                }
                Err(error) => {
                    if sandbox.read_bytes(path).await.is_err() {
                        record_absent(&observation, path);
                    }
                    Err(error)
                }
            }
        })
    }
}

/// write：写沙箱内文本文件（自动建父目录）。
/// 完整 schema 始终下发（deferred 已停用）；工具面由当前 Agent 决定。
/// 是否需要人工审批由部署侧 ApprovalGate 策略决定，不写进工具定义。
/// read-before-write：覆盖既有文件前须已读过；
/// 创建新文件不受限（缺失观察允许创建）。
pub struct WriteTool {
    pub sandbox: Arc<Sandbox>,
    pub observation: Arc<FsObservation>,
}

impl ToolPort for WriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write".to_string(),
            description: "Write text content to a file under the sandbox root (parent directories \
                created automatically). Existing files are fully replaced. Input: {\"file_path\": string, \
                \"content\": string}."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "relative file path under sandbox root" },
                    "content": { "type": "string", "description": "text content to write" }
                },
                "required": ["file_path", "content"]
            }),
        }
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::WorkspaceWrite
    }

    fn defer_loading(&self) -> bool {
        // 2026-10-20：deferred 保留代码但不使用——工具默认完整披露。
        false
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let sandbox = self.sandbox.clone();
        let observation = self.observation.clone();
        Box::pin(async move {
            let path = call.input.get("file_path").and_then(Value::as_str).unwrap_or("");
            let content = call.input.get("content").and_then(Value::as_str).unwrap_or("");
            // 覆盖既有文件（create/fully-replace 语义）须先读过；新建文件本就无可读
            // 内容，直接放行（write 创建新文件不受 read-before-write 约束）。
            let current = match sandbox.read_bytes(path).await {
                Ok(data) => Some(version_of(&data)),
                Err(_) => None,
            };
            if current.is_some() {
                let key = key_of(std::path::Path::new(path));
                observation.guard(&key, current.as_deref())?;
            }
            let message = sandbox.write(path, content).await?;
            // 写后本会话已持有新事实，刷新观测。
            if let Ok(data) = sandbox.read_bytes(path).await {
                record_present(&observation, path, &data);
            }
            Ok(vec![ContentBlock::Text { text: message }])
        })
    }
}
