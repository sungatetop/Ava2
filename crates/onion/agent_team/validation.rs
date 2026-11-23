//! agent-team 输入校验 + 归一化。
//!
//! - `member_name`：小写 kebab-case，len<=64，`!= "lead"`；
//! - `required_text`：trim 后非空 + 长度上限；
//! - `write_scope`：反斜杠转正斜杠、去 `./` 前缀、去尾部 `/`、禁止绝对路径/
//!   盘符/空段/`.`/`..`。

use regex::Regex;

use super::TeamError;

/// 成员名正则（小写 kebab-case）。
const MEMBER_NAME_RE: &str = r"^[a-z0-9]+(?:-[a-z0-9]+)*$";
const MEMBER_NAME_MAX: usize = 64;
/// 保留的 Lead 名（成员不可命名 `lead`）。
const LEAD_NAME: &str = "lead";

/// 校验成员名。
pub fn member_name(name: &str) -> Result<(), TeamError> {
    let ok = Regex::new(MEMBER_NAME_RE).is_ok_and(|re| re.is_match(name))
        && name.len() <= MEMBER_NAME_MAX
        && name != LEAD_NAME;
    if ok {
        Ok(())
    } else {
        Err(TeamError::InvalidMemberName(name.to_string()))
    }
}

/// 校验必填文本（trim 非空 + 长度上限）。
pub fn required_text(value: &str, max: usize) -> Result<String, TeamError> {
    let text = value.trim();
    if text.is_empty() || text.len() > max {
        return Err(TeamError::InvalidArgument("required text".into()));
    }
    Ok(text.to_string())
}

/// 归一化并校验一个写范围前缀。
pub fn write_scope(value: &str) -> Result<String, TeamError> {
    // 反斜杠转正斜杠、去尾部 `/`、去 `./` 前缀。
    let mut s = value.replace('\\', "/");
    while s.ends_with('/') {
        s.pop();
    }
    while s.starts_with("./") {
        s = s[2..].to_string();
    }
    // 禁止绝对路径 / 盘符 / 空串。
    if s.is_empty()
        || s.starts_with('/')
        || s.len() >= 2 && s.as_bytes()[1] == b':' && s.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err(TeamError::InvalidWriteScope(value.to_string()));
    }
    // 禁止空段 / `.` / `..`。
    for segment in s.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(TeamError::InvalidWriteScope(value.to_string()));
        }
    }
    Ok(s)
}

/// 归一化写范围列表（去重保序）。
pub fn write_scopes(values: &[String]) -> Result<Vec<String>, TeamError> {
    let mut out = Vec::new();
    for v in values {
        let normalized = write_scope(v)?;
        if !out.contains(&normalized) {
            out.push(normalized);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
