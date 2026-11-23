//! 会话查询的文本过滤与游标分页（非 FTS 路径文本匹配 + base64url 游标）。
//!
//! onion 不引入 SQLite/FTS5（语料为工作区级小型 JSONL，线性扫描足够）：文本匹配
//! 取「字面量 + 大小写不敏感 + 词间空白弹性」语义，配合非重叠匹配计数支撑
//! `match_count DESC, document_length ASC, time DESC` 排序。游标为
//! `base64url(JSON{v,f,g,o})`：请求指纹不符 → Invalid cursor（参数变了，偏移失义），
//! 语料代际不符 → Stale cursor（corpus 变更，须重跑）。

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::SessionFile;

/// 编译后的文本过滤：查询词序列（小写）。
pub struct TextFilter {
    words: Vec<String>,
}

impl TextFilter {
    /// 编译查询（调用前查询已经 `normalize_query` 归一，词数 ≥ 1）。
    pub fn compile(query: &str) -> Self {
        Self { words: query.split_whitespace().map(str::to_lowercase).collect() }
    }

    /// 非重叠匹配计数：文档按顺序出现全部词且词间为（长度弹性的）空白间隔，
    /// 记一次匹配。`None` = 不匹配；`Some(n)` = 匹配 n 次（排序键 match_count）。
    pub fn match_count(&self, doc: &str) -> Option<usize> {
        let first = self.words.first()?;
        let lower = doc.to_lowercase();
        let mut from = 0usize;
        let mut count = 0usize;
        while let Some(start) = lower[from..].find(first.as_str()) {
            let head = from + start + first.len();
            match match_rest(&lower, &self.words[1..], head) {
                Some(end) => {
                    count += 1;
                    from = end;
                }
                None => from = head, // 该出现未能构成完整短语，跳过继续找
            }
        }
        (count > 0).then_some(count)
    }
}

/// 从 `pos` 起匹配剩余词：词间要求至少一个（长度弹性的）空白。
/// `pos` 必须落在字符边界上（调用方以词长推进保证）。
fn match_rest(doc: &str, rest: &[String], mut pos: usize) -> Option<usize> {
    for word in rest {
        let tail = doc.get(pos..)?;
        let ws = tail.len() - tail.trim_start().len();
        if ws == 0 {
            return None;
        }
        pos += ws;
        let tail = doc.get(pos..)?;
        if !tail.starts_with(word.as_str()) {
            return None;
        }
        pos += word.len();
    }
    Some(pos)
}

/// 游标负载：格式版本 + 请求指纹 + 语料代际 + 行偏移。
#[derive(Serialize, Deserialize)]
pub struct SearchCursor {
    pub v: u32,
    pub f: String,
    pub g: String,
    pub o: u64,
}

/// 游标格式版本（负载形态变更时递增，旧游标一律判 Invalid）。
pub const CURSOR_VERSION: u32 = 1;

/// 游标编码：base64url(JSON)（无填充，URL 安全）。
pub fn encode_cursor(cursor: &SearchCursor) -> String {
    let json = serde_json::to_string(cursor).unwrap_or_default();
    URL_SAFE_NO_PAD.encode(json.as_bytes())
}

/// 游标解码；载荷不合法或版本不符 → Err（`INVALID_CURSOR`）。
pub fn decode_cursor(token: &str) -> Result<SearchCursor, String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(token.trim())
        .map_err(|_| "Invalid cursor: not a valid cursor token; re-run the search".to_string())?;
    let cursor: SearchCursor = serde_json::from_slice(&bytes)
        .map_err(|_| "Invalid cursor: malformed cursor payload; re-run the search".to_string())?;
    if cursor.v != CURSOR_VERSION {
        return Err("Invalid cursor: unsupported cursor version; re-run the search".to_string());
    }
    Ok(cursor)
}

/// sha256 前 16 字节的 hex（64 位指纹空间；请求指纹与语料代际共用）。
pub fn stable_digest(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    digest[..16].iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 校验游标与当前请求指纹 / 语料代际一致，返回行偏移。
pub fn validate_cursor(
    cursor: Option<&SearchCursor>,
    fingerprint: &str,
    generation: &str,
) -> Result<u64, String> {
    let Some(cursor) = cursor else { return Ok(0) };
    if cursor.f != fingerprint {
        return Err("Invalid cursor: search parameters changed since the cursor was issued; re-run the search without it".to_string());
    }
    if cursor.g != generation {
        return Err("Stale cursor: session corpus changed since the cursor was issued; re-run the search".to_string());
    }
    Ok(cursor.o)
}

/// 跨会话语料代际描述符：候选会话按 id 排序后的 `id:文件长度` 串接。
/// 任何会话文件的增删或追加都会改变描述符 → 在途游标判 Stale。
pub fn corpus_descriptor(files: &[SessionFile]) -> String {
    let mut parts: Vec<String> = files
        .iter()
        .map(|file| {
            let len = std::fs::metadata(&file.path).map(|meta| meta.len()).unwrap_or(0);
            format!("{}:{len}", file.id)
        })
        .collect();
    parts.sort();
    parts.join("|")
}

#[cfg(test)]
#[path = "filters_tests.rs"]
mod tests;
