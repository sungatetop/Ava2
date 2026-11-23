//! stdio 帧读写：JSON-RPC 2.0 over stdio（新行分隔帧）。
//!
//! 行式 JSON-RPC 传输：每请求/响应/通知为一行 JSON（`\n` 分隔），
//! 无需 `Content-Length` 头（行即帧边界）。stdout 只承载协议帧——引擎日志
//! 一律走 stderr，保证 stdout 纯净（外部 SDK client 只读 stdout）。

use std::io::{self, BufRead, Write};

/// 从 stdin 读取一行帧（返回去掉行尾换行的原始字符串）。
pub fn read_frame<R: BufRead>(reader: &mut R) -> io::Result<Option<String>> {
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Ok(None);
    }
    let line = line.trim_end_matches(['\r', '\n']).to_string();
    Ok(Some(line))
}

/// 向 stdout 写入一行帧（追加 `\n`），立即 flush。
pub fn write_frame<W: Write>(writer: &mut W, line: &str) -> io::Result<()> {
    writeln!(writer, "{line}")?;
    writer.flush()
}
