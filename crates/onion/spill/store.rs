//! spill 本地 FS 后端：
//! 会话子目录隔离 + 路径安全编码 + 随机前缀防碰撞 + 互斥创建。

use std::io::Write;
use std::path::PathBuf;

use super::SESSIONS_DIR;

/// 缺省溢出阈值（估算 token）：超过才落 spill 工件
/// （保守量级——常规超长结果仍由 E1 视图级裁剪兜底，spill 只接超大件）。
pub const DEFAULT_MAX_INLINE_TOKENS: u64 = 20_000;

/// 编码段最大字符数（截断仅损失可读性，唯一性由随机前缀保证）。
const MAX_SEGMENT_CHARS: usize = 48;

/// spill 工件定位符 + 取回提示。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpillRef {
    /// 不透明定位符（本地后端 = 工件绝对路径；模型经 read 工具读回）。
    pub locator: String,
    /// 工件字节数。
    pub bytes: u64,
    /// 给模型的取回提示。
    pub retrieval_hint: String,
}

/// spill 工件库（本地 FS 后端）：`<root>/sessions/<encoded-session-id>/` 布局。
pub struct SpillStore {
    root: PathBuf,
    /// 溢出阈值（估算 token）；0 = 禁用保留（存取与 GC 仍可用）。
    max_inline_tokens: u64,
}

impl SpillStore {
    pub fn new(root: PathBuf, max_inline_tokens: u64) -> Self {
        Self { root, max_inline_tokens }
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// 溢出阈值（估算 token）；0 = 禁用保留。
    pub fn max_inline_tokens(&self) -> u64 {
        self.max_inline_tokens
    }

    /// 保存文本工件：`<root>/sessions/<encoded-session-id>/<随机前缀>-<编码名>.txt`。
    /// 互斥创建（`create_new`）：并发保存不覆盖既有工件。
    pub fn save_text(
        &self,
        owner_session_id: &str,
        suggested_name: &str,
        content: &str,
    ) -> std::io::Result<SpillRef> {
        let session_dir = self.root.join(SESSIONS_DIR).join(encode_segment(owner_session_id));
        std::fs::create_dir_all(&session_dir)?;
        let file_name = format!("{}-{}.txt", random_prefix(), encode_segment(suggested_name));
        let path = session_dir.join(file_name);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(content.as_bytes())?;
        let locator = path.display().to_string();
        Ok(SpillRef {
            bytes: content.len() as u64,
            retrieval_hint: format!("Read the file at {locator} to see the complete output."),
            locator,
        })
    }
}

/// 路径安全段编码（单射编码）：字母数字与 `._-`
/// 保留，其余字节转义为 `~XX`（十六进制大写，`~` 自身必被转义故无歧义）；
/// 空段退化为 `_`；超长截断（唯一性由随机前缀保证，截断不破坏正确性）。
fn encode_segment(raw: &str) -> String {
    let mut out = String::new();
    for byte in raw.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(byte as char),
            _ => out.push_str(&format!("~{byte:02X}")),
        }
    }
    if out.is_empty() {
        return "_".to_string();
    }
    if out.chars().count() > MAX_SEGMENT_CHARS {
        out.chars().take(MAX_SEGMENT_CHARS).collect()
    } else {
        out
    }
}

/// 12 位十六进制随机前缀（uuid v4 去连字符取首段）：同建议名并发保存不碰撞。
fn random_prefix() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
