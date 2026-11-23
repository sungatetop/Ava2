//! 数据目录解析：全局 home 路径 + 会话持久化 JSONL 布局。
//!
//! - 数据根：`ONION_HOME`（空字符串视为未设置）> `~/.onion`；全 crate 唯一根解析点，
//!   其他模块禁止自行定义数据根（根解析收敛于此单点）。
//! - 会话布局：`<root>/sessions/<normalized-cwd>/<encoded-session-id>/session.v1.jsonl`
//!   —— 项目目录隔离 + 会话独立目录 + 版本化文件名。会话文件不落工作空间。
//! - 配置落位：
//!   - 全局配置 `<root>/config.toml`（等价 ava `<AVA_HOME>/config.toml`）
//!   - 凭据 `<root>/.credentials`（权限 0600）
//!   - 工作空间本地覆盖 `<workspace>/.onion/`（显式提交物，等价 ava `.ava/`）

use std::path::{Path, PathBuf};

/// 数据根（唯一解析点）：`ONION_HOME`（空视为未设置）> `~/.onion`。
pub fn data_home() -> PathBuf {
    data_home_from(
        std::env::var("ONION_HOME").ok().as_deref(),
        home_dir().as_deref(),
    )
}

/// 跨平台用户主目录（Windows `USERPROFILE` / Unix `HOME`）。
/// 供第三方 skills 兼容目录（`~/.claude/skills`、`~/.agents/skills`、`~/.dsh/skills`）等
/// home 级路径解析使用。
pub fn user_home() -> Option<PathBuf> {
    home_dir()
}

/// [`data_home`] 的纯函数形态（env 与 home 由调用方注入，便于测试；
/// 空白 env 视为未设置）。
pub fn data_home_from(env_value: Option<&str>, home: Option<&Path>) -> PathBuf {
    match env_value {
        Some(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => home.unwrap_or_else(|| Path::new(".")).join(".onion"),
    }
}

/// 全局配置路径：`<root>/config.toml`。
pub fn config_file(root: &Path) -> PathBuf {
    root.join("config.toml")
}

/// 凭据文件路径：`<root>/.credentials`。
pub fn credentials_file(root: &Path) -> PathBuf {
    root.join(".credentials")
}

/// 用户级 settings 文件路径：`<root>/settings.json`（/model 切换持久化）。
pub fn settings_file(root: &Path) -> PathBuf {
    root.join("settings.json")
}

/// 日志目录：`<root>/logs`。
pub fn logs_dir(root: &Path) -> PathBuf {
    root.join("logs")
}

/// 后台任务输出 spill 路径：`<root>/logs/jobs/<job_id>.log`
/// （超限输出 spill 到文件；等价 ava `<root>/jobs/<session>/<job>.log`）。
pub fn job_log_file(root: &Path, job_id: &str) -> PathBuf {
    logs_dir(root).join("jobs").join(format!("{}.log", encode_session_id(job_id)))
}

/// spill 工件根：`<root>/spill/`（超大工具结果持久化，本地文件布局；
/// 内含 `sessions/<会话 id>/` 子树，按 mtime 启动清扫 GC）。
pub fn spill_dir(root: &Path) -> PathBuf {
    root.join("spill")
}

/// 子代理会话日志根：`<root>/logs/subagents/`（内含 `<encoded-id>/session.vN.jsonl`）。
///
/// 子代理**不按 cwd 分区**（同一子会话可能工作在父会话之外的工作目录），
/// 因此与父会话的 `sessions/<cwd>/` 树并列，直接按会话 id 寻址。装配侧
/// （onion-server）与读面（`session_query`）共用本函数，避免字面量重复约定。
pub fn subagent_logs_dir(root: &Path) -> PathBuf {
    logs_dir(root).join("subagents")
}

/// 附件存储根：`<root>/attachments/v1`（内容寻址对象 + 文件别名，见 `attachment`）。
pub fn attachments_dir(root: &Path) -> PathBuf {
    root.join("attachments").join("v1")
}

/// 实例标识（env `ONION_INSTANCE_ID`，缺省 `default`）：AgentCard、`GET /v1/meta`
/// 与委派链接（`reportTo`）共用同一来源。
pub fn instance_id() -> String {
    std::env::var("ONION_INSTANCE_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "default".to_string())
}

/// 实例显示名（env `ONION_INSTANCE_NAME`，缺省回落 [`instance_id`]）：作为 A2A
/// 出站消息的 `metadata.sender.name`，供对端区分「谁在说话」。
pub fn instance_name() -> String {
    std::env::var("ONION_INSTANCE_NAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(instance_id)
}

/// 项目级会话容器目录：`<root>/sessions/<normalized-cwd>/`。
pub fn sessions_dir(root: &Path, cwd: &Path) -> PathBuf {
    root.join("sessions").join(normalize_cwd(cwd))
}

/// 单会话目录：`<sessions_dir>/<encoded-session-id>/`（每会话独立目录）。
pub fn session_dir(sessions_dir: &Path, session_id: &str) -> PathBuf {
    sessions_dir.join(encode_session_id(session_id))
}

/// 单会话日志文件：`<session_dir>/session.v1.jsonl`（版本化文件名）。
pub fn session_log_file(session_dir: &Path) -> PathBuf {
    session_dir.join(format!("session.v{SESSION_LOG_VERSION}.jsonl"))
}

/// 会话日志文件版本号（`session.vN.jsonl` 的版本位）。
pub const SESSION_LOG_VERSION: u32 = 1;

/// 会话文件名稳定前缀（枚举/回放按此前缀定位当前版本文件）。
pub const SESSION_LOG_PREFIX: &str = "session.v";

/// cwd 安全转义（归一化可读形态）：路径分隔符 / 盘符冒号 / 空白
/// 统一替换为 `-`，其余字符原样保留。
pub fn normalize_cwd(cwd: &Path) -> String {
    escape_component(&cwd.to_string_lossy())
}

/// 会话 id 安全转义（编码形态）：仅保留字母数字与 `-`/`_`，
/// 其余（含 `.`）统一替换为 `_`，杜绝路径穿越。
pub fn encode_session_id(session_id: &str) -> String {
    let escaped: String = session_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' })
        .collect();
    // 空 id（理论上不可达）退化为占位名，避免映射到容器目录本身。
    if escaped.is_empty() { "session".to_string() } else { escaped }
}

fn escape_component(raw: &str) -> String {
    raw.chars()
        .map(|c| if matches!(c, ':' | '\\' | '/' | ' ' | '\t') { '-' } else { c })
        .collect()
}

/// 跨平台用户主目录（Windows `USERPROFILE` / Unix `HOME`）。
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
