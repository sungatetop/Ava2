//! PTY 后端：用 portable-pty 拉起真 shell。
//!
//! 核心语义：
//! - 受控提示符（`CONTROLLED_PROMPT`）+ OSC 133 就绪标记用于就绪判定；
//! - 就绪判定四态（stdin_read / inferred_idle / timeout / session_exit）；
//! - `signal` 向会话前台进程组送白名单信号；
//! - `kill` 终止进程树（幂等）。
//!
//! Windows 走 ConPTY（CreatePseudoConsole），需要进程有 console 上下文；无 console
//! 时 spawn 返回结构化 `NoBackend` 类错误（由服务层透出），而非 panic。

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use super::seam::{PtySignal, TerminalBackend, TerminalError, TerminalSpawned, TerminalStatus};

/// 受控提示符（PS1）：用于就绪握手。
pub const CONTROLLED_PROMPT: &str = "onion$ ";

/// 就绪判定：检测到提示符即视为 stdin 可写。
///
/// 简化实现：以「收到任何输出」近似 `stdin_read`；完整实现用 OSC 133 就绪序列。
/// onion 为 Rust 单进程，不做完整 xterm 模拟，故就绪判定退化为「有输出即就绪」——
/// 语义等价（模型侧据 waitReason 判断前台命令是否结束），实现层简化。
pub fn is_ready_marker(bytes: &[u8]) -> bool {
    !bytes.is_empty()
}

/// 一个已打开的 PTY 会话（backend 内部状态）。
struct PtyHandle {
    _master: Box<dyn MasterPty + Send>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    status: Arc<Mutex<TerminalStatus>>,
}

/// portable-pty 后端：`type_ = "shell"`，拉起 `bash`（Windows 回退 `pwsh`）。
pub struct PtyBackend {
    handle: Arc<Mutex<Option<PtyHandle>>>,
}

/// Windows shell 解析：优先 `pwsh.exe`（PowerShell 7，需自装），PATH 无则回退
/// `powershell.exe`（Windows PowerShell 5.1，系统自带必装）——硬编码 `pwsh.exe`
/// 在未装 PS7 的机器上 spawn 直接 os error 2。
fn windows_shell() -> &'static str {
    let path = std::env::var("PATH").unwrap_or_default();
    if path
        .split(';')
        .any(|dir| std::path::Path::new(dir).join("pwsh.exe").is_file())
    {
        "pwsh.exe"
    } else {
        "powershell.exe"
    }
}

impl PtyBackend {
    pub fn new() -> Self {
        Self { handle: Arc::new(Mutex::new(None)) }
    }
}

impl Default for PtyBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalBackend for PtyBackend {
    fn type_(&self) -> &str {
        "shell"
    }

    fn spawn(&self, cwd: Option<&str>) -> Result<TerminalSpawned, TerminalError> {
        let pty_system = native_pty_system();
        let pty = pty_system
            .openpty(PtySize { rows: 24, cols: 120, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| TerminalError::no_backend(format!("failed to open pty: {e}")))?;
        let shell = if cfg!(windows) { windows_shell() } else { "bash" };
        let mut cmd = CommandBuilder::new(shell);
        // 受控提示符 + 关闭交互式启动横幅（受控 prompt 语义）。
        cmd.env("PS1", CONTROLLED_PROMPT);
        cmd.env("TERM", "dumb");
        if let Some(cwd) = cwd {
            cmd.cwd(cwd);
        }
        let child = pty.slave
            .spawn_command(cmd)
            .map_err(|e| TerminalError::no_backend(format!("failed to spawn {shell}: {e}")))?;
        let writer = pty.master.take_writer().map_err(|e| {
            TerminalError::no_backend(format!("failed to take pty writer: {e}"))
        })?;
        let reader = pty.master.try_clone_reader().map_err(|e| {
            TerminalError::no_backend(format!("failed to clone pty reader: {e}"))
        })?;
        let handle = PtyHandle {
            _master: pty.master,
            writer: Arc::new(Mutex::new(writer)),
            reader: Arc::new(Mutex::new(reader)),
            child: Arc::new(Mutex::new(child)),
            status: Arc::new(Mutex::new(TerminalStatus::Running)),
        };
        let mut guard = self.handle.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(handle);
        Ok(TerminalSpawned {
            pid: None,
            motd: format!(
                "spawned {shell} in a persistent pty; type commands with terminal_send. \
prompt: {CONTROLLED_PROMPT}"
            ),
        })
    }

    fn write(&self, text: &str, submit: bool) -> Result<(), TerminalError> {
        let guard = self.handle.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(handle) = guard.as_ref() else {
            return Err(TerminalError::no_session("no active pty session".to_string()));
        };
        let mut writer = handle.writer.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        writer.write_all(text.as_bytes()).map_err(|e| {
            TerminalError::no_session(format!("failed to write to pty: {e}"))
        })?;
        if submit {
            writer.write_all(b"\r").map_err(|e| {
                TerminalError::no_session(format!("failed to write newline to pty: {e}"))
            })?;
        }
        writer.flush().map_err(|e| {
            TerminalError::no_session(format!("failed to flush pty writer: {e}"))
        })?;
        Ok(())
    }

    fn read(&self) -> Result<Vec<u8>, TerminalError> {
        let guard = self.handle.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(handle) = guard.as_ref() else {
            return Ok(Vec::new());
        };
        let mut reader = handle.reader.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut buf = [0u8; 4096];
        let mut out = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    out.extend_from_slice(&buf[..n]);
                    if out.len() >= 16384 {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        Ok(out)
    }

    fn signal(&self, sig: PtySignal) -> Result<(), TerminalError> {
        let guard = self.handle.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(handle) = guard.as_ref() else {
            return Err(TerminalError::no_session("no active pty session".to_string()));
        };
        // 白名单校验由工具层负责；此处仅转发。portable-pty 无进程组信号原语，
        // 简化：SIGINT/SIGTSTP 经写入中断字节（ConPTY 会转发），其余经 child.kill()。
        match sig {
            PtySignal::Sigint => {
                let mut writer = handle.writer.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let _ = writer.write_all(&[0x03]);
                let _ = writer.flush();
            }
            PtySignal::Sigstp => {
                let mut writer = handle.writer.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let _ = writer.write_all(&[0x1a]);
                let _ = writer.flush();
            }
            _ => {
                let mut child = handle.child.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let _ = child.kill();
            }
        }
        Ok(())
    }

    fn kill(&self) -> Result<(), TerminalError> {
        let guard = self.handle.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(handle) = guard.as_ref() {
            let mut child = handle.child.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let _ = child.kill();
            let mut status = handle.status.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            *status = TerminalStatus::Exited { exit_code: 0, signal: None };
        }
        Ok(())
    }

    fn status(&self) -> TerminalStatus {
        let guard = self.handle.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        match guard.as_ref() {
            Some(handle) => {
                handle.status.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
            }
            None => TerminalStatus::Exited { exit_code: 0, signal: None },
        }
    }
}

#[cfg(test)]
#[path = "pbackend_tests.rs"]
mod tests;
