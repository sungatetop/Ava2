//! 跨进程会话写租约。
//!
//! 以 OS 级内核锁（POSIX flock / Windows 命名信号量）保证同一会话日志
//! 同时只有一个写者：持有者进程死亡即自动释放，绝无「陈旧锁阻塞后继者」。
//! 锁粒度为单会话目录：
//!
//! - **Windows**：`session.lock` 以 `GENERIC_WRITE` 打开且共享模式仅
//!   `FILE_SHARE_READ | FILE_SHARE_DELETE`——第二个写者打开时命中内核共享
//!   冲突（ERROR_SHARING_VIOLATION）；只读读者与目录删除（share DELETE）
//!   仍可继续，不阻塞读者/目录移除。
//! - **POSIX**：`flock(fd, LOCK_EX | LOCK_NB)`。
//!
//! 约束：锁文件只创建、从不主动删除（删除会话目录前必须先经
//! [`crate::session::SessionLog::close`] 释放租约）；持有者进程死亡时内核
//! 自动回收句柄/描述符，锁即时释放。

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// 租约获取失败原因。
#[derive(Debug)]
pub enum SessionLeaseError {
    /// 该会话已被其他进程（或本进程另一写者）持有。
    AlreadyOwned,
    /// 打开/加锁的底层 IO 失败。
    Io(io::Error),
}

impl std::fmt::Display for SessionLeaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyOwned => f.write_str("写租约已被其他持有者占用"),
            Self::Io(err) => write!(f, "写租约获取失败：{err}"),
        }
    }
}

/// 跨进程写租约。持有 `session.lock` 的句柄/描述符，Drop 即释放。
#[derive(Debug)]
pub struct SessionWriteLease {
    /// Windows 上持有句柄即持有内核共享冲突；POSIX 上持有 fd 即持有 flock。
    file: File,
}

impl SessionWriteLease {
    /// 锁文件名。
    pub const FILE_NAME: &'static str = "session.lock";

    /// 在 `dir` 下获取 `<dir>/session.lock` 的独占写租约（非阻塞）。
    pub fn acquire(dir: &Path) -> Result<Self, SessionLeaseError> {
        Self::acquire_at(&dir.join(Self::FILE_NAME))
    }

    /// 以完整锁文件路径获取独占写租约（非阻塞）。
    ///
    /// [`Self::acquire`] 的通用形态：会话日志之外的
    /// 任意跨进程读改写文档（如凭据文件 `.credentials.lock`）都以一份独立锁
    /// 文件保护，语义与持有规则完全相同。
    pub fn acquire_at(lock_path: &Path) -> Result<Self, SessionLeaseError> {
        let file = open_lease_file(lock_path)?;
        try_lock_exclusive(&file)?;
        Ok(Self { file })
    }
}

impl Drop for SessionWriteLease {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // SAFETY: flock 仅作用于自持的合法 fd；fd 在 Drop 后关闭即彻底释放。
            let fd = std::os::unix::io::AsRawFd::as_raw_fd(&self.file);
            unsafe { libc::flock(fd, libc::LOCK_UN) };
        }
        #[cfg(windows)]
        {
            // Windows 无需显式操作：句柄关闭即解除内核共享冲突。
            let _ = &self.file;
        }
    }
}

/// Windows `ERROR_SHARING_VIOLATION`（`winerror.h`）：目标文件正被以不兼容
/// 的共享模式打开——即写租约已被持有。
const ERROR_SHARING_VIOLATION: i32 = 32;

/// 打开锁文件本体（只创建/复用，不截断、不写入内容）。
///
/// Windows 专用：仅共享「读 + 删除」——拒绝第二个写者（`GENERIC_WRITE` 不在
/// 共享集内，命中 [`ERROR_SHARING_VIOLATION`]），读者与目录删除不受阻。
#[cfg(windows)]
fn open_lease_file(path: &Path) -> Result<File, SessionLeaseError> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_DELETE: u32 = 0x4;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(false);
    options.share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE);
    options.open(path).map_err(|err| {
        if err.raw_os_error() == Some(ERROR_SHARING_VIOLATION) {
            SessionLeaseError::AlreadyOwned
        } else {
            SessionLeaseError::Io(err)
        }
    })
}

/// 打开锁文件本体（只创建/复用，不截断、不写入内容）。
#[cfg(unix)]
fn open_lease_file(path: &Path) -> Result<File, SessionLeaseError> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(false);
    options.open(path).map_err(SessionLeaseError::Io)
}

/// 在已打开的锁文件上尝试独占（非阻塞）；冲突即 [`SessionLeaseError::AlreadyOwned`]。
#[cfg(unix)]
fn try_lock_exclusive(file: &File) -> Result<(), SessionLeaseError> {
    use std::os::unix::io::AsRawFd;
    // SAFETY: flock 仅作用于自持的合法 fd，非阻塞模式不会挂起调用线程。
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        return Ok(());
    }
    let os = io::Error::last_os_error();
    // EWOULDBLOCK 与 EAGAIN 在多数平台同值，避免同值常量模式重复告警，走比较。
    let code = os.raw_os_error().unwrap_or(0);
    if code == libc::EWOULDBLOCK || code == libc::EAGAIN {
        return Err(SessionLeaseError::AlreadyOwned);
    }
    Err(SessionLeaseError::Io(os))
}

/// Windows 上排他在 `open_lease_file` 的共享模式冲突中一次性完成。
#[cfg(windows)]
fn try_lock_exclusive(_file: &File) -> Result<(), SessionLeaseError> {
    Ok(())
}

#[cfg(test)]
#[path = "session_lock_tests.rs"]
mod tests;
