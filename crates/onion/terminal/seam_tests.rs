//! `seam` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use onion_jobs::SessionId;

use crate::terminal::seam::{
    PtySignal, TerminalBackend, TerminalErrorCode, TerminalService, TerminalSpawned,
    TerminalStatus, MAX_SCROLLBACK_LINES,
};

/// 测试后端桩：记录已送信号 / 写文本，可配置状态。
struct FakeBackend {
    type_: &'static str,
    writes: std::sync::Mutex<Vec<String>>,
    signals: std::sync::Mutex<Vec<PtySignal>>,
    status: std::sync::Mutex<TerminalStatus>,
    read_data: std::sync::Mutex<Vec<u8>>,
}

impl FakeBackend {
    fn new(type_: &'static str) -> Self {
        Self {
            type_,
            writes: std::sync::Mutex::new(Vec::new()),
            signals: std::sync::Mutex::new(Vec::new()),
            status: std::sync::Mutex::new(TerminalStatus::Running),
            read_data: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl TerminalBackend for FakeBackend {
    fn type_(&self) -> &str {
        self.type_
    }
    fn spawn(&self, _cwd: Option<&str>) -> Result<TerminalSpawned, crate::terminal::seam::TerminalError> {
        Ok(TerminalSpawned { pid: Some(123), motd: "fake motd".to_string() })
    }
    fn write(&self, text: &str, _submit: bool) -> Result<(), crate::terminal::seam::TerminalError> {
        self.writes.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(text.to_string());
        Ok(())
    }
    fn read(&self) -> Result<Vec<u8>, crate::terminal::seam::TerminalError> {
        let mut data = self.read_data.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(std::mem::take(&mut *data))
    }
    fn signal(&self, sig: PtySignal) -> Result<(), crate::terminal::seam::TerminalError> {
        self.signals.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(sig);
        Ok(())
    }
    fn kill(&self) -> Result<(), crate::terminal::seam::TerminalError> {
        *self.status.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            TerminalStatus::Exited { exit_code: 0, signal: None };
        Ok(())
    }
    fn status(&self) -> TerminalStatus {
        self.status.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }
}

fn owner(id: &str) -> SessionId {
    SessionId(id.to_string())
}

#[test]
fn id_minting_is_monotonic() {
    let svc = TerminalService::new();
    let backend: Arc<dyn TerminalBackend> = Arc::new(FakeBackend::new("shell"));
    svc.register_backend(backend).unwrap();
    let a = svc.spawn(owner("o1"), "shell", None, None).unwrap();
    let b = svc.spawn(owner("o1"), "shell", None, None).unwrap();
    assert_eq!(a.0, "pty-1");
    assert_eq!(b.0, "pty-2");
}

#[test]
fn register_backend_conflict_rejected() {
    let svc = TerminalService::new();
    let a: Arc<dyn TerminalBackend> = Arc::new(FakeBackend::new("shell"));
    let b: Arc<dyn TerminalBackend> = Arc::new(FakeBackend::new("shell"));
    svc.register_backend(a).unwrap();
    let err = svc.register_backend(b).unwrap_err();
    assert_eq!(err.code, TerminalErrorCode::DuplicateBackend);
}

#[test]
fn register_backend_empty_type_rejected() {
    let svc = TerminalService::new();
    let backend: Arc<dyn TerminalBackend> = Arc::new(FakeBackend::new(""));
    let err = svc.register_backend(backend).unwrap_err();
    assert_eq!(err.code, TerminalErrorCode::DuplicateBackend);
}

#[test]
fn spawn_no_backend_returns_error() {
    let svc = TerminalService::new();
    let err = svc.spawn(owner("o1"), "shell", None, None).unwrap_err();
    assert_eq!(err.code, TerminalErrorCode::NoBackend);
}

#[test]
fn spawn_duplicate_name_rejected() {
    let svc = TerminalService::new();
    let backend: Arc<dyn TerminalBackend> = Arc::new(FakeBackend::new("shell"));
    svc.register_backend(backend).unwrap();
    svc.spawn(owner("o1"), "shell", Some("main"), None).unwrap();
    let err = svc.spawn(owner("o1"), "shell", Some("main"), None).unwrap_err();
    assert_eq!(err.code, TerminalErrorCode::DuplicateName);
}

#[test]
fn foreign_session_rejected() {
    let svc = TerminalService::new();
    let backend: Arc<dyn TerminalBackend> = Arc::new(FakeBackend::new("shell"));
    svc.register_backend(backend).unwrap();
    let id = svc.spawn(owner("o1"), "shell", None, None).unwrap();
    let err = svc.read(&owner("o2"), &id, 0, 10).unwrap_err();
    assert_eq!(err.code, TerminalErrorCode::ForeignSession);
}

#[test]
fn send_is_exclusive() {
    let svc = TerminalService::new();
    let backend = Arc::new(FakeBackend::new("shell"));
    svc.register_backend(backend).unwrap();
    let id = svc.spawn(owner("o1"), "shell", None, None).unwrap();
    svc.start_send(&owner("o1"), &id, "echo hi", true).unwrap();
    let err = svc.start_send(&owner("o1"), &id, "echo again", true).unwrap_err();
    assert_eq!(err.code, TerminalErrorCode::SendActive);
    // 结束释放独占。
    svc.finish_send(&id);
    svc.start_send(&owner("o1"), &id, "echo again", true).unwrap();
}

#[test]
fn read_absorbs_backend_output_and_paginates() {
    let svc = TerminalService::new();
    let backend = Arc::new(FakeBackend::new("shell"));
    svc.register_backend(backend.clone()).unwrap();
    let id = svc.spawn(owner("o1"), "shell", None, None).unwrap();
    *backend.read_data.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
        b"line1\nline2\nline3\n".to_vec();
    let lines = svc.read(&owner("o1"), &id, 0, 2).unwrap();
    assert_eq!(lines, vec!["line1".to_string(), "line2".to_string()]);
    let rest = svc.read(&owner("o1"), &id, 2, 10).unwrap();
    assert_eq!(rest, vec!["line3".to_string()]);
}

#[test]
fn kill_is_idempotent() {
    let svc = TerminalService::new();
    let backend = Arc::new(FakeBackend::new("shell"));
    svc.register_backend(backend).unwrap();
    let id = svc.spawn(owner("o1"), "shell", None, None).unwrap();
    svc.kill(&owner("o1"), &id).unwrap();
    svc.kill(&owner("o1"), &id).unwrap();
}

#[test]
fn no_session_returns_error() {
    let svc = TerminalService::new();
    let err = svc.read(&owner("o1"), &crate::terminal::seam::TerminalSessionId("pty-99".to_string()), 0, 1).unwrap_err();
    assert_eq!(err.code, TerminalErrorCode::NoSession);
}

#[test]
fn dispose_owner_cleans_sessions() {
    let svc = TerminalService::new();
    let backend: Arc<dyn TerminalBackend> = Arc::new(FakeBackend::new("shell"));
    svc.register_backend(backend).unwrap();
    let id = svc.spawn(owner("o1"), "shell", None, None).unwrap();
    assert_eq!(svc.list(&owner("o1")).len(), 1);
    svc.dispose_owner(&owner("o1"));
    assert_eq!(svc.list(&owner("o1")).len(), 0);
    assert!(svc.read(&owner("o1"), &id, 0, 1).is_err());
}

#[test]
fn list_scoped_to_owner() {
    let svc = TerminalService::new();
    let backend: Arc<dyn TerminalBackend> = Arc::new(FakeBackend::new("shell"));
    svc.register_backend(backend).unwrap();
    svc.spawn(owner("o1"), "shell", None, None).unwrap();
    svc.spawn(owner("o2"), "shell", None, None).unwrap();
    assert_eq!(svc.list(&owner("o1")).len(), 1);
    assert_eq!(svc.list(&owner("o2")).len(), 1);
}

#[test]
fn scrollback_is_bounded() {
    let svc = TerminalService::new();
    let backend = Arc::new(FakeBackend::new("shell"));
    svc.register_backend(backend.clone()).unwrap();
    let id = svc.spawn(owner("o1"), "shell", None, None).unwrap();
    let mut data = Vec::new();
    for i in 0..(MAX_SCROLLBACK_LINES + 100) {
        data.extend_from_slice(format!("l{i}\n").as_bytes());
    }
    *backend.read_data.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = data;
    let lines = svc.read(&owner("o1"), &id, 0, usize::MAX).unwrap();
    assert_eq!(lines.len(), MAX_SCROLLBACK_LINES);
}
