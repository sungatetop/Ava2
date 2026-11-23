//! Codex 风格多文件补丁：解析 + 纯文本应用。
//!
//! 移植自 OpenAI Codex
//! rust-v0.158.0 的 parser.rs / seek_sequence.rs / file_update.rs：
//! - 语法：`*** Begin Patch` … `*** End Patch`，内含 Add / Update / Delete File
//!   头，Update 可带 `*** Move to:` 与 `@@` 锚点、`*** End of File`；
//! - 应用：hunk 按序在原文上定位（多轮归一化匹配：原样 → 去尾空白 → 去首尾空白
//!   → 标点归一化），任一 hunk 失配即整体失败且不返回部分结果。
//!
//! 本模块**纯函数、无 IO**：路径解析、沙箱遏制、观测门禁与落盘由工具壳
//! （`tools/patch.rs`）负责。

use std::fmt;

const BEGIN: &str = "*** Begin Patch";
const END: &str = "*** End Patch";
const EOF_MARKER: &str = "*** End of File";
const MOVE: &str = "*** Move to: ";
const ADD: &str = "*** Add File: ";
const DELETE: &str = "*** Delete File: ";
const UPDATE: &str = "*** Update File: ";

/// 补丁语法错误（带一基行号，便于模型定位）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchParseError {
    pub line_number: usize,
    pub reason: String,
}

impl fmt::Display for PatchParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid patch at line {}: {}", self.line_number, self.reason)
    }
}

impl std::error::Error for PatchParseError {}

/// 一个连续更新块。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchHunk {
    /// 一基输入行号（标记行或首个隐式 hunk 行）。
    pub line_number: usize,
    /// `@@ ` 之后的文本，作为搜索锚点。
    pub context: Option<String>,
    /// 上下文 + 删除行（不含补丁前缀与行尾）。
    pub old_lines: Vec<String>,
    /// 上下文 + 新增行（不含补丁前缀与行尾）。
    pub new_lines: Vec<String>,
    /// 本 hunk 之后是否跟随 `*** End of File`。
    pub is_end_of_file: bool,
}

/// 有序文件操作；move 是带目标路径的 update。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatchOperation {
    Add { path: String, line_number: usize, content: String },
    Delete { path: String, line_number: usize },
    Update {
        path: String,
        line_number: usize,
        move_to: Option<String>,
        hunks: Vec<PatchHunk>,
    },
}

impl PatchOperation {
    /// 操作涉及的源路径。
    pub fn path(&self) -> &str {
        match self {
            Self::Add { path, .. } | Self::Delete { path, .. } | Self::Update { path, .. } => path,
        }
    }

    /// 操作的一基补丁行号。
    pub fn line_number(&self) -> usize {
        match self {
            Self::Add { line_number, .. }
            | Self::Delete { line_number, .. }
            | Self::Update { line_number, .. } => *line_number,
        }
    }

    /// move 目标路径（仅 Update 可能携带）。
    pub fn move_to(&self) -> Option<&str> {
        match self {
            Self::Update { move_to, .. } => move_to.as_deref(),
            _ => None,
        }
    }
}

/// 首个失配 hunk 的诊断。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HunkApplyFailure {
    /// 失配 hunk 的一基补丁行号。
    pub hunk_line_number: usize,
    /// 失配阶段（锚点 / 旧内容）。
    pub stage: HunkStage,
    /// 顺序搜索游标（EOF 搜索覆盖为尾部候选）。
    pub search_start: usize,
    /// 原始锚点或旧内容行。
    pub pattern: Vec<String>,
}

/// 失配阶段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HunkStage {
    Context,
    OldLines,
}

impl HunkStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::OldLines => "oldLines",
        }
    }
}

/// 应用结果：完整新文本，或首个失配（不含部分结果）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyHunksResult {
    Ok { after: String },
    Failed { failure: HunkApplyFailure },
}

// ---------------- 解析 ----------------

/// 解析完整 Codex 风格补丁（不读写文件）。
///
/// 空补丁返回空表；语法错误返回 [`PatchParseError`]。
pub fn parse_patch(patch: &str) -> Result<Vec<PatchOperation>, PatchParseError> {
    let lines = patch_lines(patch)?;
    let mut operations: Vec<PatchOperation> = Vec::new();
    let mut ended = false;
    let last_index = lines.len().saturating_sub(1);
    for (index, (text, number)) in lines.iter().enumerate().skip(1) {
        let is_last = index == last_index;
        // update 模式下末行标记同样被 trim（对齐 Codex batch-parser finish）。
        let header = match operations.last() {
            Some(PatchOperation::Update { .. }) if !is_last => trim_end(text),
            _ => trim(text),
        };
        if ended && !is_last {
            if !trim(text).is_empty() {
                return Err(PatchParseError {
                    line_number: *number,
                    reason: format!("Unexpected content after '{END}'"),
                });
            }
            continue;
        }
        if header == END {
            ensure_update(&operations, *number)?;
            ended = true;
        } else if let Some(path) = header.strip_prefix(ADD) {
            ensure_update(&operations, *number)?;
            operations.push(PatchOperation::Add {
                path: path.to_string(),
                line_number: *number,
                content: String::new(),
            });
        } else if let Some(path) = header.strip_prefix(DELETE) {
            ensure_update(&operations, *number)?;
            operations.push(PatchOperation::Delete {
                path: path.to_string(),
                line_number: *number,
            });
        } else if let Some(path) = header.strip_prefix(UPDATE) {
            ensure_update(&operations, *number)?;
            operations.push(PatchOperation::Update {
                path: path.to_string(),
                line_number: *number,
                move_to: None,
                hunks: Vec::new(),
            });
        } else if operations.is_empty() && header.starts_with("*** Environment ID:") {
            return Err(PatchParseError {
                line_number: *number,
                reason: "Environment ID headers are unsupported; the caller selects the filesystem provider"
                    .to_string(),
            });
        } else if let Some(PatchOperation::Add { content, .. }) = operations.last_mut() {
            if let Some(rest) = text.strip_prefix('+') {
                content.push_str(rest);
                content.push('\n');
            } else {
                return Err(PatchParseError {
                    line_number: *number,
                    reason: "Expected an Add File, Delete File, or Update File header; added content must start with +"
                        .to_string(),
                });
            }
        } else if let Some(PatchOperation::Update { .. }) = operations.last() {
            read_update(&mut operations, text, *number)?;
        } else {
            return Err(PatchParseError {
                line_number: *number,
                reason: "Expected an Add File, Delete File, or Update File header; added content must start with +"
                    .to_string(),
            });
        }
    }
    Ok(operations)
}

/// 切分补丁行并校验 Begin / End 包裹（支持 heredoc 包裹）。
fn patch_lines(patch: &str) -> Result<Vec<(String, usize)>, PatchParseError> {
    let leading = patch.len() - patch.trim_start().len();
    let offset = patch[..leading].matches('\n').count();
    let mut lines: Vec<(String, usize)> = trim(patch)
        .split('\n')
        .enumerate()
        .map(|(index, text)| (text.trim_end_matches('\r').to_string(), offset + index + 1))
        .collect();
    // heredoc 包裹：`<<EOF` … `EOF`。
    if lines.len() >= 4 {
        let first = lines[0].0.clone();
        let last = lines[lines.len() - 1].0.clone();
        if matches!(first.as_str(), "<<EOF" | "<<'EOF'" | "<<\"EOF\"") && last.ends_with("EOF") {
            lines = lines[1..lines.len() - 1].to_vec();
        }
    }
    let Some((first_text, first_number)) = lines.first() else {
        return Err(PatchParseError {
            line_number: 1,
            reason: format!("Expected '{BEGIN}'"),
        });
    };
    if trim(first_text) != BEGIN {
        return Err(PatchParseError {
            line_number: *first_number,
            reason: format!("Expected '{BEGIN}'"),
        });
    }
    let (last_text, last_number) = &lines[lines.len() - 1];
    if trim(last_text) != END {
        return Err(PatchParseError {
            line_number: *last_number,
            reason: format!("Expected '{END}'"),
        });
    }
    Ok(lines)
}

/// 校验上一个 update 操作至少有一个非空 hunk。
fn ensure_update(
    operations: &[PatchOperation],
    line_number: usize,
) -> Result<(), PatchParseError> {
    let Some(PatchOperation::Update { path, line_number: op_line, hunks, .. }) = operations.last()
    else {
        return Ok(());
    };
    let Some(last) = hunks.last() else {
        return Err(PatchParseError {
            line_number: *op_line,
            reason: format!("Update for '{path}' requires at least one hunk"),
        });
    };
    if empty_hunk(last) {
        return Err(PatchParseError {
            line_number,
            reason: "Update hunk must contain context, added, or removed lines".to_string(),
        });
    }
    Ok(())
}

fn empty_hunk(hunk: &PatchHunk) -> bool {
    hunk.old_lines.is_empty() && hunk.new_lines.is_empty()
}

fn new_hunk(line_number: usize) -> PatchHunk {
    PatchHunk {
        line_number,
        context: None,
        old_lines: Vec::new(),
        new_lines: Vec::new(),
        is_end_of_file: false,
    }
}

/// 读取 update 操作内的一行（锚点 / EOF 标记 / 内容行）。
fn read_update(
    operations: &mut [PatchOperation],
    text: &str,
    number: usize,
) -> Result<(), PatchParseError> {
    let Some(PatchOperation::Update { move_to, hunks, .. }) = operations.last_mut() else {
        return Ok(());
    };
    let trimmed = trim_end(text);
    let previous_eof = hunks.last().map(|hunk| hunk.is_end_of_file).unwrap_or(false);
    let marker = trimmed == "@@" || trimmed.starts_with("@@ ");
    if previous_eof {
        if trimmed.is_empty() {
            return Ok(());
        }
        if !marker {
            return Err(PatchParseError {
                line_number: number,
                reason: "Expected '@@' after '*** End of File'".to_string(),
            });
        }
    }
    if hunks.is_empty() && move_to.is_none() {
        if let Some(destination) = trimmed.strip_prefix(MOVE) {
            *move_to = Some(destination.to_string());
            return Ok(());
        }
    }
    if marker {
        if let Some(previous) = hunks.last() {
            if empty_hunk(previous) {
                return Err(PatchParseError {
                    line_number: number,
                    reason: "Update hunk must contain context, added, or removed lines".to_string(),
                });
            }
        }
        let mut hunk = new_hunk(number);
        if let Some(context) = trimmed.strip_prefix("@@ ") {
            hunk.context = Some(context.to_string());
        }
        hunks.push(hunk);
        return Ok(());
    }
    if trimmed == EOF_MARKER {
        if let Some(previous) = hunks.last_mut() {
            if empty_hunk(previous) {
                return Err(PatchParseError {
                    line_number: number,
                    reason: "Update hunk must contain context, added, or removed lines".to_string(),
                });
            }
            previous.is_end_of_file = true;
        }
        return Ok(());
    }
    let prefix = text.chars().next();
    if !text.is_empty() && !matches!(prefix, Some(' ') | Some('+') | Some('-')) {
        return Err(PatchParseError {
            line_number: number,
            reason: "Expected a context (' '), added ('+'), removed ('-') line, or '@@'".to_string(),
        });
    }
    if hunks.is_empty() {
        hunks.push(new_hunk(number));
    }
    let Some(hunk) = hunks.last_mut() else {
        unreachable!("hunk pushed above when empty");
    };
    let content = text.chars().skip(1).collect::<String>();
    match prefix {
        Some('+') => hunk.new_lines.push(content),
        Some('-') => hunk.old_lines.push(content),
        _ => {
            hunk.old_lines.push(content.clone());
            hunk.new_lines.push(content);
        }
    }
    Ok(())
}

// ---------------- 应用 ----------------

/// 把已解析 hunk 应用到 LF 归一化文本（无 IO、不改入参）。
///
/// 全部搜索基于原文；任一 hunk 失配即返回失败且不含部分结果。成功时非空输出
/// 以 LF 结尾（含原本未终止的文本）。
pub fn apply_hunks(before: &str, hunks: &[PatchHunk]) -> ApplyHunksResult {
    let mut original: Vec<String> = before.split('\n').map(str::to_string).collect();
    if original.last().map(String::as_str) == Some("") {
        original.pop();
    }
    let mut replacements: Vec<Replacement> = Vec::new();
    let mut line_index = 0usize;

    for hunk in hunks {
        if let Some(context) = &hunk.context {
            let pattern = vec![context.clone()];
            let Some(found) = seek_sequence(&original, &pattern, line_index, false) else {
                return ApplyHunksResult::Failed {
                    failure: HunkApplyFailure {
                        hunk_line_number: hunk.line_number,
                        stage: HunkStage::Context,
                        search_start: line_index,
                        pattern,
                    },
                };
            };
            line_index = found + 1;
        }
        if hunk.old_lines.is_empty() {
            let start = if original.last().map(String::as_str) == Some("") {
                original.len() - 1
            } else {
                original.len()
            };
            replacements.push(Replacement {
                start,
                old_length: 0,
                new_lines: hunk.new_lines.clone(),
            });
            continue;
        }

        let mut pattern = hunk.old_lines.clone();
        let mut new_lines = hunk.new_lines.clone();
        let mut found = seek_sequence(&original, &pattern, line_index, hunk.is_end_of_file);
        if found.is_none() && pattern.last().map(String::as_str) == Some("") {
            pattern.pop();
            if new_lines.last().map(String::as_str) == Some("") {
                new_lines.pop();
            }
            found = seek_sequence(&original, &pattern, line_index, hunk.is_end_of_file);
        }
        let Some(found) = found else {
            return ApplyHunksResult::Failed {
                failure: HunkApplyFailure {
                    hunk_line_number: hunk.line_number,
                    stage: HunkStage::OldLines,
                    search_start: line_index,
                    pattern: hunk.old_lines.clone(),
                },
            };
        };
        replacements.push(Replacement {
            start: found,
            old_length: pattern.len(),
            new_lines,
        });
        line_index = found + pattern.len();
    }

    // 稳定排序同时保留同源插入点的补丁顺序。
    replacements.sort_by_key(|replacement| replacement.start);
    let mut lines = original;
    for replacement in replacements.iter().rev() {
        let tail = replacement.start + replacement.old_length;
        lines = lines[..replacement.start]
            .iter()
            .cloned()
            .chain(replacement.new_lines.iter().cloned())
            .chain(lines[tail.min(lines.len())..].iter().cloned())
            .collect();
    }
    if lines.last().map(String::as_str) != Some("") {
        lines.push(String::new());
    }
    ApplyHunksResult::Ok { after: lines.join("\n") }
}

struct Replacement {
    start: usize,
    old_length: usize,
    new_lines: Vec<String>,
}

/// 多轮归一化匹配（对齐 Codex `seek_sequence`）：原样 → 去尾空白 → 去首尾空白
/// → 标点归一化。EOF 搜索只检查尾部候选。
fn seek_sequence(
    lines: &[String],
    pattern: &[String],
    start: usize,
    eof: bool,
) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }
    if pattern.len() > lines.len() {
        return None;
    }
    let last = lines.len() - pattern.len();
    let first = if eof { last } else { start };
    for normalize in MATCH_PASSES {
        let expected: Vec<String> = pattern.iter().map(|line| normalize(line)).collect();
        for index in first..=last {
            if expected
                .iter()
                .enumerate()
                .all(|(offset, want)| normalize(&lines[index + offset]) == *want)
            {
                return Some(index);
            }
        }
    }
    None
}

type Normalize = fn(&str) -> String;

const MATCH_PASSES: [Normalize; 4] = [
    |value| value.to_string(),
    |value| trim_whitespace(value, false),
    |value| trim_whitespace(value, true),
    normalize_punctuation,
];

fn trim_whitespace(value: &str, leading: bool) -> String {
    let trimmed = if leading { value.trim_start() } else { value };
    trimmed.trim_end().to_string()
}

/// 标点归一化（对齐 Codex `normalize_punctuation`）：各类破折号 → `-`、弯引号 →
/// 直引号、各类不间断/全角空格 → 普通空格。
fn normalize_punctuation(value: &str) -> String {
    trim_whitespace(value, true)
        .chars()
        .map(|c| match c {
            '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
            '\u{2018}'..='\u{201B}' => '\'',
            '\u{201C}'..='\u{201F}' => '"',
            '\u{00A0}' | '\u{2002}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' => ' ',
            other => other,
        })
        .collect()
}

fn trim(value: &str) -> &str {
    value.trim()
}

fn trim_end(value: &str) -> &str {
    value.trim_end()
}

#[cfg(test)]
#[path = "patch_tests.rs"]
mod tests;
