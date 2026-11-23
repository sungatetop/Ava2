//! Terminal seam 类型与 [`TerminalService`]。
//!
//! 词汇约定：会话以 `pty-{n}` 单调 id 铸造；owner 以 onion-jobs 的 `SessionId` 标识，
//! 会话只对发起它的 agent 可见/可操作（`FOREIGN_SESSION` 拒绝）。seam 不暴露协议类型 /
//! 进程句柄，只暴露六种语义操作（spawn / read / send / signal / kill / list）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use onion_jobs::SessionId;

/// 会话滚动缓冲上限（行）。有界缓冲，`terminal_read` 分页读取；超限丢弃最旧行。
pub const MAX_SCROLLBACK_LINES: usize = 10_000;

/// 会话标识（`pty-{n}`，服务内单调铸造）。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TerminalSessionId(pub String);

impl std::fmt::Display for TerminalSessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// 会话生命周期状态。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalStatus {
    /// 进程在飞。
    Running,
    /// 已退出。
    Exited { exit_code: i32, signal: Option<PtySignal> },
}

/// 前台命令结束的等待原因（模型面必须看到，用于判断命令是否真结束）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitReason {
    /// 检测到受控提示符（stdin 可写）。
    StdinRead,
    /// 输出静默推断空闲。
    InferredIdle,
    /// 绝对超时。
    Timeout,
    /// 会话进程退出。
    SessionExit,
}

impl WaitReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            WaitReason::StdinRead => "stdin_read",
            WaitReason::InferredIdle => "inferred_idle",
            WaitReason::Timeout => "timeout",
            WaitReason::SessionExit => "session_exit",
        }
    }

    pub const ALL: [WaitReason; 4] = [
        WaitReason::StdinRead,
        WaitReason::InferredIdle,
        WaitReason::Timeout,
        WaitReason::SessionExit,
    ];

    pub fn parse(value: &str) -> Option<WaitReason> {
        Self::ALL.iter().copied().find(|r| r.as_str() == value)
    }
}

/// 可送信号白名单。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PtySignal {
    Sigint,
    Sigterm,
    Sigkill,
    Sigstp,
    Sighup,
}

impl PtySignal {
    pub fn as_str(&self) -> &'static str {
        match self {
            PtySignal::Sigint => "SIGINT",
            PtySignal::Sigterm => "SIGTERM",
            PtySignal::Sigkill => "SIGKILL",
            PtySignal::Sigstp => "SIGTSTP",
            PtySignal::Sighup => "SIGHUP",
        }
    }

    pub const ALL: [PtySignal; 5] = [
        PtySignal::Sigint,
        PtySignal::Sigterm,
        PtySignal::Sigkill,
        PtySignal::Sigstp,
        PtySignal::Sighup,
    ];

    pub fn parse(value: &str) -> Option<PtySignal> {
        let value = value.to_ascii_uppercase();
        Self::ALL.iter().copied().find(|s| s.as_str() == value)
    }
}

/// 结构化 terminal 失败。带稳定 `code`（调用方据此路由，而非解析 message）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalError {
    pub message: String,
    pub code: TerminalErrorCode,
}

impl std::fmt::Display for TerminalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TerminalError {}

/// 稳定错误码。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalErrorCode {
    /// 同名 backend 类型重复注册。
    DuplicateBackend,
    /// 会话名在会话集内重复。
    DuplicateName,
    /// 会话不属于该 owner。
    ForeignSession,
    /// 无 backend 处理该类型。
    NoBackend,
    /// 会话不存在（或已被清理）。
    NoSession,
    /// owner 已不存活（会话清理期）。
    OwnerNotLive,
    /// 已有 send 在飞，独占发送。
    SendActive,
    /// 服务正在 dispose。
    ServiceDisposing,
}

impl TerminalError {
    pub fn duplicate_backend(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: TerminalErrorCode::DuplicateBackend }
    }
    pub fn duplicate_name(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: TerminalErrorCode::DuplicateName }
    }
    pub fn foreign_session(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: TerminalErrorCode::ForeignSession }
    }
    pub fn no_backend(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: TerminalErrorCode::NoBackend }
    }
    pub fn no_session(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: TerminalErrorCode::NoSession }
    }
    pub fn send_active(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: TerminalErrorCode::SendActive }
    }
}

/// spawn 产物：进程 pid（不可得时 None）+ 会话 MOTD。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSpawned {
    pub pid: Option<u32>,
    pub motd: String,
}

/// PTY 后端（注册于 [`TerminalService`]，按 `type_` 唯一）。onion 用 portable-pty
/// 拉起真 shell（S14b）；S14a 以抽象 trait 支撑 seam 纯逻辑测试。
pub trait TerminalBackend: Send + Sync {
    /// 稳定后端类型标识（如 `"shell"`），服务按此路由 spawn。
    fn type_(&self) -> &str;
    /// 启动一个会话进程。实现方负责就绪握手与 MOTD 生成。
    fn spawn(&self, cwd: Option<&str>) -> Result<TerminalSpawned, TerminalError>;
    /// 写输入到会话 stdin。
    fn write(&self, text: &str, submit: bool) -> Result<(), TerminalError>;
    /// 读取会话输出（原始字节；服务侧维护 scrollback）。
    fn read(&self) -> Result<Vec<u8>, TerminalError>;
    /// 向会话前台进程组送信号。
    fn signal(&self, sig: PtySignal) -> Result<(), TerminalError>;
    /// 终止会话进程树。
    fn kill(&self) -> Result<(), TerminalError>;
    /// 会话当前状态。
    fn status(&self) -> TerminalStatus;
}

/// 一个会话（服务持有，跨 backend 抽象）。
struct TerminalSession {
    id: TerminalSessionId,
    owner: SessionId,
    name: String,
    type_: String,
    status: TerminalStatus,
    motd: String,
    /// 有界滚动缓冲（行）。
    scrollback: Vec<String>,
    /// 独占发送标志。
    send_active: bool,
    backend: Arc<dyn TerminalBackend>,
}

impl TerminalSession {
    fn view(&self) -> TerminalSessionView {
        TerminalSessionView {
            id: self.id.clone(),
            name: self.name.clone(),
            type_: self.type_.clone(),
            status: self.status.clone(),
            motd: self.motd.clone(),
            lines: self.scrollback.len(),
        }
    }
}

/// 会话对外不可变视图（`terminal_list` 渲染用）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSessionView {
    pub id: TerminalSessionId,
    pub name: String,
    pub type_: String,
    pub status: TerminalStatus,
    pub motd: String,
    pub lines: usize,
}

/// Terminal 能力 seam。owner 隔离的会话注册表：
/// backend 按 `type_` 唯一注册，spawn 铸造 `pty-{n}` id，会话只对 owner 可见。
#[derive(Clone, Default)]
pub struct TerminalService {
    inner: Arc<Mutex<TerminalServiceInner>>,
}

#[derive(Default)]
struct TerminalServiceInner {
    session_counter: u64,
    backends: HashMap<String, Arc<dyn TerminalBackend>>,
    sessions: HashMap<TerminalSessionId, Arc<Mutex<TerminalSession>>>,
    disposing: bool,
}

impl TerminalService {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册 backend，按 `type_` 唯一；冲突抛 [`TerminalError`]（`DuplicateBackend`）。
    pub fn register_backend(&self, backend: Arc<dyn TerminalBackend>) -> Result<(), TerminalError> {
        let type_ = backend.type_().to_string();
        if type_.trim().is_empty() {
            return Err(TerminalError::duplicate_backend(
                "a terminal backend type must be a non-empty string",
            ));
        }
        let mut inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if inner.backends.contains_key(&type_) {
            return Err(TerminalError::duplicate_backend(format!(
                "a terminal backend with type \"{type_}\" is already registered"
            )));
        }
        inner.backends.insert(type_, backend);
        Ok(())
    }

    /// 是否有已注册 backend（供装配 / 冒烟诊断）。
    pub fn has_backend(&self, type_: &str) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .backends
            .contains_key(type_)
    }

    /// 启动会话：按 `type_` 路由 backend，铸造 `pty-{n}` id，名称预留（与会话集
    /// 内重复拒绝）。owner 隔离：会话归属 `owner`。
    pub fn spawn(
        &self,
        owner: SessionId,
        type_: &str,
        name: Option<&str>,
        cwd: Option<&str>,
    ) -> Result<TerminalSessionId, TerminalError> {
        let mut inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if inner.disposing {
            return Err(TerminalError { message: "terminal service is disposing".to_string(), code: TerminalErrorCode::ServiceDisposing });
        }
        let backend = inner
            .backends
            .get(type_)
            .cloned()
            .ok_or_else(|| TerminalError::no_backend(format!("no terminal backend for type \"{type_}\"")))?;
        if let Some(name) = name {
            if inner.sessions.values().any(|s| {
                s.lock().unwrap_or_else(std::sync::PoisonError::into_inner).name == name
            }) {
                return Err(TerminalError::duplicate_name(format!(
                    "a terminal session named \"{name}\" already exists"
                )));
            }
        }
        let spawned = backend.spawn(cwd)?;
        inner.session_counter += 1;
        let id = TerminalSessionId(format!("pty-{}", inner.session_counter));
        let session = Arc::new(Mutex::new(TerminalSession {
            id: id.clone(),
            owner,
            name: name.map(str::to_string).unwrap_or_else(|| id.0.clone()),
            type_: type_.to_string(),
            status: TerminalStatus::Running,
            motd: spawned.motd,
            scrollback: Vec::new(),
            send_active: false,
            backend,
        }));
        inner.sessions.insert(id.clone(), session);
        Ok(id)
    }

    /// 独占发送：向会话写输入。已有 send 在飞抛 [`TerminalError`]（`SendActive`）。
    /// `submit` 为 true 时追加换行提交。
    pub fn start_send(
        &self,
        owner: &SessionId,
        id: &TerminalSessionId,
        text: &str,
        submit: bool,
    ) -> Result<(), TerminalError> {
        let session = self.session_owned(owner, id)?;
        let mut session = session.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if session.send_active {
            return Err(TerminalError::send_active(format!(
                "terminal session {} already has an active send",
                session.id
            )));
        }
        session.backend.write(text, submit)?;
        session.send_active = true;
        Ok(())
    }

    /// 标示一次 send 结束（读环/等待结束后调用），释放独占。
    pub fn finish_send(&self, id: &TerminalSessionId) {
        if let Some(session) =
            self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner).sessions.get(id)
        {
            let mut session = session.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            session.send_active = false;
        }
    }

    /// 分页读取会话滚动缓冲：吸收 backend 输出进 scrollback，再按 `offset`/`count`
    /// 返回行切片。
    pub fn read(
        &self,
        owner: &SessionId,
        id: &TerminalSessionId,
        offset: usize,
        count: usize,
    ) -> Result<Vec<String>, TerminalError> {
        let session = self.session_owned(owner, id)?;
        let mut session = session.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // 吸收 backend 输出（原始字节 → 行），维护有界缓冲。
        if let Ok(bytes) = session.backend.read() {
            let text = String::from_utf8_lossy(&bytes);
            let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
            if !text.ends_with('\n') && !lines.is_empty() {
                // 无尾换行：末行可能不完整，先并入缓冲末行。
                if let (Some(last), Some(existing)) = (lines.pop(), session.scrollback.last_mut()) {
                    existing.push_str(&last);
                }
            }
            session.scrollback.extend(lines);
            if session.scrollback.len() > MAX_SCROLLBACK_LINES {
                let excess = session.scrollback.len() - MAX_SCROLLBACK_LINES;
                session.scrollback.drain(..excess);
            }
        }
        let total = session.scrollback.len();
        let begin = offset.min(total);
        let end = (begin + count).min(total);
        Ok(session.scrollback[begin..end].to_vec())
    }

    /// 向会话前台进程组送信号（白名单校验由工具层负责）。
    pub fn signal(
        &self,
        owner: &SessionId,
        id: &TerminalSessionId,
        sig: PtySignal,
    ) -> Result<(), TerminalError> {
        let session = self.session_owned(owner, id)?;
        let guard = session.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.backend.signal(sig)
    }

    /// 终止会话进程树（幂等：已结束也返回 Ok）。
    pub fn kill(&self, owner: &SessionId, id: &TerminalSessionId) -> Result<(), TerminalError> {
        let session = self.session_owned(owner, id)?;
        let mut session = session.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        session.backend.kill()?;
        session.status = TerminalStatus::Exited { exit_code: 0, signal: None };
        Ok(())
    }

    /// 列出某 owner 的全部会话视图（`terminal_list` 渲染用）。
    pub fn list(&self, owner: &SessionId) -> Vec<TerminalSessionView> {
        let inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut views: Vec<TerminalSessionView> = inner
            .sessions
            .values()
            .filter(|s| {
                s.lock().unwrap_or_else(std::sync::PoisonError::into_inner).owner == *owner
            })
            .map(|s| s.lock().unwrap_or_else(std::sync::PoisonError::into_inner).view())
            .collect();
        views.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        views
    }

    /// owner 生命周期清理：移除其全部会话（agent dispose 连带）。
    pub fn dispose_owner(&self, owner: &SessionId) {
        let mut inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let owned: Vec<TerminalSessionId> = inner
            .sessions
            .iter()
            .filter(|(_, s)| s.lock().unwrap_or_else(std::sync::PoisonError::into_inner).owner == *owner)
            .map(|(id, _)| id.clone())
            .collect();
        for id in owned {
            if let Some(session) = inner.sessions.remove(&id) {
                let _ = session.lock().unwrap_or_else(std::sync::PoisonError::into_inner).backend.kill();
            }
        }
    }

    /// 取 owner 拥有的会话（`FOREIGN_SESSION` / `NO_SESSION` 校验）。
    fn session_owned(
        &self,
        owner: &SessionId,
        id: &TerminalSessionId,
    ) -> Result<Arc<Mutex<TerminalSession>>, TerminalError> {
        let inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let session = inner
            .sessions
            .get(id)
            .cloned()
            .ok_or_else(|| TerminalError::no_session(format!("terminal session {id} not found")))?;
        {
            let guard = session.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if guard.owner != *owner {
                return Err(TerminalError::foreign_session(format!(
                    "terminal session {id} is owned by another session"
                )));
            }
        }
        Ok(session)
    }
}

#[cfg(test)]
#[path = "seam_tests.rs"]
mod tests;
