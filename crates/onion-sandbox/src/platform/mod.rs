//! 平台分发的沙箱执行器。
//!
//! Windows：真实 OS 隔离（受限 token + 路径 ACL + Job Object，见 `windows` 子模块）。
//! 其它平台：预留 seam，明确返回「未实现」而非静默透传，避免误以为已隔离。

#[cfg(windows)]
mod windows;

use crate::{SandboxExecRequest, SandboxExecResult};

/// 在 OS 沙箱内执行请求。失败返回错误描述（非沙箱拒绝）。
pub fn run_sandboxed(request: &SandboxExecRequest) -> Result<SandboxExecResult, String> {
    #[cfg(windows)]
    {
        windows::run_sandboxed(request)
    }
    #[cfg(not(windows))]
    {
        // 预留：Linux 可复刻 ava-linux-sandbox（bwrap + landlock + seccomp），
        // macOS 可复刻 ava-macos-sandbox（seatbelt）。当前明确报「未实现」。
        let _ = request;
        Err(format!(
            "onion-sandbox: OS sandbox not implemented on this platform (only Windows currently)"
        ))
    }
}
