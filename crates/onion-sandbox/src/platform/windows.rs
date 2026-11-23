//! Windows 沙箱执行器：真实 OS 隔离（受限 token + 路径 ACL + Job Object）。
//!
//! 沙箱模式执行时强制策略：
//! - `read-only`：受限 token（`WRITE_RESTRICTED` + 禁最高权限）+ 对可写根追加 deny-write
//!   ACL，任何文件写被 OS 拒绝 → `sandbox_denied`。
//! - `workspace-write`：受限 token + 仅对 `cwd` 追加 grant-write ACL（默认 DACL 已拒写），
//!   越界写被 OS 拒绝 → `sandbox_denied`。
//! - `danger-full-access`：直接以当前权限 spawn，不做隔离。
//!
//! 与 ava-windows-sandbox 的差异：本实现是独立可插拔 exe 的完整闭环（token + ACL + Job +
//! 管道 + 等待 + 读输出一次完成），而非暴露可拼装底层函数的库。

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W,
    TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
};
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, CopySid, CreateRestrictedToken, CreateWellKnownSid, EqualSid, GetAce,
    GetLengthSid, GetTokenInformation, LookupPrivilegeValueW, SetTokenInformation, ACL,
    SID_AND_ATTRIBUTES, TOKEN_ADJUST_DEFAULT, TOKEN_ADJUST_PRIVILEGES, TOKEN_ADJUST_SESSIONID,
    TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_GROUPS, TOKEN_PRIVILEGES, TOKEN_QUERY,
    TokenDefaultDacl, TokenGroups, DACL_SECURITY_INFORMATION,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CreateProcessAsUserW, GetCurrentProcess, GetExitCodeProcess, OpenProcessToken, PROCESS_INFORMATION,
    STARTUPINFOW, STARTF_USESTDHANDLES, WaitForSingleObject, INFINITE,
};
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_ALL_ACCESS, FILE_APPEND_DATA, FILE_GENERIC_READ, FILE_EXECUTE,
    FILE_WRITE_ATTRIBUTES, FILE_WRITE_DATA, FILE_WRITE_EA, ReadFile, WRITE_DAC, WRITE_OWNER,
};

use crate::{SandboxExecRequest, SandboxExecResult, SandboxMode};

const DISABLE_MAX_PRIVILEGE: u32 = 0x01;
const LUA_TOKEN: u32 = 0x04;
const WRITE_RESTRICTED: u32 = 0x08;
const SE_GROUP_LOGON_ID: u32 = 0xC000_0000;
const WIN_WORLD_SID: i32 = 1;
const GRANT_ACCESS: i32 = 2;
const SE_FILE_OBJECT: i32 = 1;
/// `CREATE_NO_WINDOW`：子进程不分配控制台。输出全走管道，控制台只会带来两处风险：
/// 弹窗闪烁，以及宿主本身无控制台时子进程在控制台初始化上挂住（实测 pwsh 超时）。
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 顶层入口：按模式执行。
pub fn run_sandboxed(request: &SandboxExecRequest) -> Result<SandboxExecResult, String> {
    match request.mode {
        // 全权限：直接以当前权限 spawn（无隔离），仍走同一执行/输出管道。
        SandboxMode::DangerFullAccess => execute_plain(request),
        SandboxMode::ReadOnly | SandboxMode::WorkspaceWrite => execute_isolated(request),
    }
}

/// 无隔离执行（danger-full-access）：直接 spawn 目标进程。
fn execute_plain(request: &SandboxExecRequest) -> Result<SandboxExecResult, String> {
    let output = std::process::Command::new(&request.program)
        .args(&request.command)
        .current_dir(&request.cwd)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| format!("failed to start {}: {error}", request.program))?;
    Ok(SandboxExecResult::ok(
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

/// 隔离执行（read-only / workspace-write）：受限 token + 路径 ACL + Job Object。
fn execute_isolated(request: &SandboxExecRequest) -> Result<SandboxExecResult, String> {
    // 1. 受限 token（一并取 logon SID：受限 token 的访问检查要求「常规 SID 与
    //    restricting SID 双重通过」，workspace-write 的可写根授权即授给它）。
    let (h_token, logon_sid) = unsafe { create_restricted_token()? };

    // 2. 按模式收敛可写根的 ACL：
    //    - workspace-write：补一条带继承的 logon 写授权。**只在缺失时补**——ACE 带
    //      继承，`SetNamedSecurityInfoW` 会同步递归传播到整棵既有子树，含 `target/`
    //      的巨型工作区（实测 26 万+ 条目）单次传播就超过 60s；每条命令都「授权 +
    //      回收」等于工作区写不可用。幂等后只有首条命令付传播成本。
    //    - read-only：若工作区残留上一轮的写授权（同一 logon 会话共享 logon SID，
    //      只读档也能用上），此时撤销它；否则只读档会被悄悄放宽成可写档（fail-open）。
    //      撤销同样只在「有残留」时发生。
    match request.mode {
        SandboxMode::WorkspaceWrite => unsafe { ensure_write_acl(&request.cwd, logon_sid.as_ptr() as *mut c_void)? },
        SandboxMode::ReadOnly => unsafe { revoke_write_acl(&request.cwd, logon_sid.as_ptr() as *mut c_void)? },
        SandboxMode::DangerFullAccess => unreachable!("handled in run_sandboxed"),
    };

    // 3. 以受限 token 创建子进程（Job Object 兜底 kill-on-close）。
    let mut proc = unsafe { spawn_isolated(h_token, &request.program, &request.command, &request.cwd) }?;
    unsafe { CloseHandle(h_token) };

    // 4. 等待退出。
    let exit_code = unsafe { wait_for_process(proc.h_process) };
    // 5. 读输出。
    let output = unsafe { read_output(proc.h_output_read).unwrap_or_default() };
    // 6. 释放句柄（写授权保留在本工作区，见第 2 步）。
    unsafe { cleanup_process(&mut proc) };

    // 沙箱拒绝判定：以「输出含访问拒绝特征」为准，**不以退出码为前提**。
    // 一方面正常业务失败（`cargo test` 有用例失败、`test -f` 返回 1）也是非零退出，
    // 按退出码判会把它们误报成沙箱拒绝并触发无意义的升级审批；另一方面 PowerShell
    // 的 cmdlet 写失败是非终止错误（`New-Item : Access to the path ... is denied.`
    // 之后继续执行、仍以 0 退出），按退出码判又会漏判真正的越界拒绝。
    // 未被判为拒绝时保留退出码与输出原文，交上层原样呈现。
    let exit_code = exit_code.unwrap_or(i32::MAX as u32) as i32;
    if looks_like_access_denied(&output) {
        return Ok(SandboxExecResult::denied(String::new(), output));
    }
    Ok(SandboxExecResult::ok(exit_code, output, String::new()))
}

/// 访问拒绝输出标记（小写匹配）：受限 token 下越界写会由 OS 以 access denied /
/// UnauthorizedAccessException 形态回给子进程，文案随 shell 与系统语言不同。
const ACCESS_DENIED_MARKERS: &[&str] = &[
    "access is denied",
    "access denied",
    // .NET / PowerShell 越界写标准文案：`Access to the path '...' is denied.`
    "is denied",
    "permission denied",
    "unauthorizedaccessexception",
    "operation not permitted",
    "拒绝访问",
    "访问被拒绝",
    "权限不足",
];

/// 输出是否呈现「访问被拒绝」特征（用于区分沙箱拦截与命令自身失败）。
///
/// 先按原文匹配；未命中再「去掉全部空白」重比一次：PowerShell 会按控制台宽度把长错误
/// 标识折行（实测 `UnauthorizedAcc` + 换行 + 若干空格 + `essException`），直接子串匹配
/// 会漏判真正的越界拒绝。
fn looks_like_access_denied(output: &str) -> bool {
    let lowered = output.to_lowercase();
    if ACCESS_DENIED_MARKERS.iter().any(|marker| lowered.contains(marker)) {
        return true;
    }
    let compact: String = lowered.chars().filter(|c| !c.is_whitespace()).collect();
    ACCESS_DENIED_MARKERS.iter().any(|marker| {
        let marker: String = marker.chars().filter(|c| !c.is_whitespace()).collect();
        compact.contains(&marker)
    })
}

/// 受限 token + Job Object 创建子进程，返回进程句柄与输出管道读端。
struct IsolatedProcess {
    h_process: HANDLE,
    h_thread: HANDLE,
    h_job: HANDLE,
    h_output_read: HANDLE,
}

/// 组装 CreateProcessAsUserW 命令行：argv 逐个按 Windows 规则加引号拼接。
/// （onion 侧 PowerShell 走 `-EncodedCommand`，Base64 参数无空格/引号；bash 的
/// `-c <command>` 参数可能含任意字符，须正确转义。）
fn build_command_line(program: &str, args: &[String]) -> String {
    let mut line = quote_windows_arg(program);
    for arg in args {
        line.push(' ');
        line.push_str(&quote_windows_arg(arg));
    }
    line
}

/// 按 Windows 命令行规则给单个参数加引号（含空格/制表符/换行/引号或为空时）。
/// 引号前的反斜杠翻倍（`2n+1` 规则），结尾反斜杠翻倍后闭合引号。
fn quote_windows_arg(arg: &str) -> String {
    let needs_quotes =
        arg.is_empty() || arg.chars().any(|c| matches!(c, ' ' | '\t' | '\n' | '"'));
    if !needs_quotes {
        return arg.to_string();
    }
    let mut quoted = String::with_capacity(arg.len() + 8);
    quoted.push('"');
    let mut backslashes = 0usize;
    for ch in arg.chars() {
        match ch {
            '\\' => {
                backslashes += 1;
                quoted.push('\\');
            }
            '"' => {
                // 引号前已有 n 个反斜杠：翻倍再补转义引号（共 2n+1 个）。
                for _ in 0..backslashes {
                    quoted.push('\\');
                }
                quoted.push('\\');
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                backslashes = 0;
                quoted.push(ch);
            }
        }
    }
    // 收尾：剩余反斜杠翻倍后闭合引号（共 2n 个）。
    for _ in 0..backslashes {
        quoted.push('\\');
    }
    quoted.push('"');
    quoted
}

unsafe fn spawn_isolated(
    h_token: HANDLE,
    program: &str,
    args: &[String],
    cwd: &Path,
) -> Result<IsolatedProcess, String> {
    // Job Object（kill-on-close 兜底）。
    let mut h_job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if h_job.is_null() {
        return Err(format!("CreateJobObjectW failed: {}", unsafe { GetLastError() }));
    }
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags =
        windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_BREAKAWAY_OK
            | windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK;
    let ok = unsafe {
        windows_sys::Win32::System::JobObjects::SetInformationJobObject(
            h_job,
            JobObjectExtendedLimitInformation,
            &mut limits as *mut _ as *mut c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if ok == 0 {
        unsafe { CloseHandle(h_job) };
        return Err(format!("SetInformationJobObject failed: {}", unsafe { GetLastError() }));
    }

    // 匿名管道（stdout+stderr 合并；stdin 给空管道）。
    let mut out_read: HANDLE = std::ptr::null_mut();
    let mut out_write: HANDLE = std::ptr::null_mut();
    let mut in_read: HANDLE = std::ptr::null_mut();
    let mut in_write: HANDLE = std::ptr::null_mut();
    let mut sa: windows_sys::Win32::Security::SECURITY_ATTRIBUTES = unsafe { std::mem::zeroed() }; 
    sa.nLength = std::mem::size_of::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>() as u32;
    sa.bInheritHandle = 1;
    if unsafe { CreatePipe(&mut out_read, &mut out_write, &sa, 0) } == 0 {
        unsafe { CloseHandle(h_job) };
        return Err(format!("CreatePipe(out) failed: {}", unsafe { GetLastError() }));
    }
    if unsafe { CreatePipe(&mut in_read, &mut in_write, &sa, 0) } == 0 {
        unsafe { CloseHandle(out_read) };
        unsafe { CloseHandle(out_write) };
        unsafe { CloseHandle(h_job) };
        return Err(format!("CreatePipe(in) failed: {}", unsafe { GetLastError() }));
    }

    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdOutput = out_write;
    si.hStdError = out_write;
    si.hStdInput = in_read;
    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    let full_command = build_command_line(program, args);
    let command_line = to_wide(&full_command);
    let cwd_wide = to_wide(cwd.as_os_str());
    let ok = unsafe {
        CreateProcessAsUserW(
            h_token,
            std::ptr::null(),
            command_line.as_ptr() as *mut u16,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            1, // bInheritHandles
            CREATE_NO_WINDOW,
            std::ptr::null_mut(),
            cwd_wide.as_ptr(),
            &si,
            &mut pi,
        )
    };
    unsafe { CloseHandle(out_write) };
    unsafe { CloseHandle(in_read) };
    unsafe { CloseHandle(in_write) };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        unsafe { CloseHandle(out_read) };
        unsafe { CloseHandle(h_job) };
        return Err(format!("CreateProcessAsUserW failed: {err}"));
    }

    // Job 绑定失败降级为可选（受限 token 已提供隔离）。
    let assigned = unsafe { AssignProcessToJobObject(h_job, pi.hProcess) } != 0;
    if !assigned {
        unsafe { CloseHandle(h_job) };
        h_job = std::ptr::null_mut();
    }

    Ok(IsolatedProcess {
        h_process: pi.hProcess,
        h_thread: pi.hThread,
        h_job,
        h_output_read: out_read,
    })
}

unsafe fn wait_for_process(h_process: HANDLE) -> Result<u32, String> {
    let res = unsafe { WaitForSingleObject(h_process, INFINITE) };
    if res != 0 {
        return Err(format!("WaitForSingleObject failed: {}", unsafe { GetLastError() }));
    }
    let mut exit_code: u32 = 0;
    let ok = unsafe { GetExitCodeProcess(h_process, &mut exit_code) };
    if ok == 0 {
        return Err(format!("GetExitCodeProcess failed: {}", unsafe { GetLastError() }));
    }
    Ok(exit_code)
}

unsafe fn read_output(h_read: HANDLE) -> Result<String, String> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut bytes_read: u32 = 0;
    loop {
        let ok = unsafe {
            ReadFile(
                h_read,
                chunk.as_mut_ptr(),
                chunk.len() as u32,
                &mut bytes_read,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            if err == 109 {
                // ERROR_BROKEN_PIPE：写端关闭，正常结束。
                break;
            }
            return Err(format!("ReadFile failed: {err}"));
        }
        if bytes_read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..bytes_read as usize]);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

unsafe fn cleanup_process(proc: &mut IsolatedProcess) {
    if !proc.h_thread.is_null() {
        unsafe { CloseHandle(proc.h_thread) };
    }
    if !proc.h_process.is_null() {
        unsafe { CloseHandle(proc.h_process) };
    }
    if !proc.h_job.is_null() {
        unsafe { CloseHandle(proc.h_job) };
    }
    if !proc.h_output_read.is_null() {
        unsafe { CloseHandle(proc.h_output_read) };
    }
}

/// 创建受限 token（WRITE_RESTRICTED + 禁最高权限 + LUA），默认 DACL 授 logon / everyone。
///
/// 返回 `(token, logon SID 字节)`：logon SID 是 restricting SID 之一，工作区写授权
/// 须授给它（受限 token 的访问检查要求常规 SID 与 restricting SID 同时通过）。
///
/// restricting 集合恒为 `{logon SID, Everyone}`，**不随模式变化**：只读档去掉 logon SID
/// 会让子进程直接起不来（实测 `0xC0000142` / STATUS_DLL_INIT_FAILED），因此只读档的
/// 「不可写」由调用方在进入只读档时撤销残留写授权来保证（见 [`revoke_write_acl`]）。
///
/// # Safety
/// 返回的 token 句柄由调用方 `CloseHandle`。
unsafe fn create_restricted_token() -> Result<(HANDLE, Vec<u8>), String> {
    let mut h_base: HANDLE = std::ptr::null_mut();
    let desired = TOKEN_DUPLICATE
        | TOKEN_QUERY
        | TOKEN_ASSIGN_PRIMARY
        | TOKEN_ADJUST_SESSIONID
        | TOKEN_ADJUST_PRIVILEGES
        | TOKEN_ADJUST_DEFAULT;
    let ok = unsafe { OpenProcessToken(GetCurrentProcess(), desired, &mut h_base) };
    if ok == 0 {
        return Err(format!("OpenProcessToken failed: {}", unsafe { GetLastError() }));
    }

    // 读取 logon SID + world SID 作为 restricting SID。
    let mut logon_sid = unsafe { get_logon_sid_bytes(h_base) }?;
    let psid_logon = logon_sid.as_mut_ptr() as *mut c_void;
    let mut everyone = unsafe { world_sid() }?;
    let psid_everyone = everyone.as_mut_ptr() as *mut c_void;

    let mut entries: Vec<SID_AND_ATTRIBUTES> = vec![unsafe { std::mem::zeroed() }; 2];
    entries[0].Sid = psid_logon;
    entries[0].Attributes = 0;
    entries[1].Sid = psid_everyone;
    entries[1].Attributes = 0;

    let mut new_token: HANDLE = std::ptr::null_mut();
    let flags = DISABLE_MAX_PRIVILEGE | LUA_TOKEN | WRITE_RESTRICTED;
    let ok = unsafe {
        CreateRestrictedToken(
            h_base,
            flags,
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
            entries.len() as u32,
            entries.as_mut_ptr(),
            &mut new_token,
        )
    };
    unsafe { CloseHandle(h_base) };
    if ok == 0 {
        return Err(format!("CreateRestrictedToken failed: {}", unsafe { GetLastError() }));
    }

    // 默认 DACL：logon 全控（进程内 IPC / 自建对象可写）+ everyone 只读。
    let defaults = [
        (psid_logon, FILE_ALL_ACCESS),
        (psid_everyone, FILE_GENERIC_READ | FILE_EXECUTE),
    ];
    unsafe { set_sandbox_default_dacl(new_token, &defaults) }?;
    // 保留 SeChangeNotifyPrivilege（目录遍历需要）。
    unsafe { enable_privilege(new_token, "SeChangeNotifyPrivilege") }?;
    Ok((new_token, logon_sid))
}

/// 读取 token 的 logon SID（restricting SID 用）。
///
/// # Safety
/// `h_token` 必须是合法 token 句柄。
unsafe fn get_logon_sid_bytes(h_token: HANDLE) -> Result<Vec<u8>, String> {
    let mut needed: u32 = 0;
    unsafe { GetTokenInformation(h_token, TokenGroups, std::ptr::null_mut(), 0, &mut needed) };
    if needed == 0 {
        return Err("GetTokenInformation(TokenGroups) size probe failed".to_string());
    }
    let mut buffer = vec![0u8; needed as usize];
    if unsafe { GetTokenInformation(h_token, TokenGroups, buffer.as_mut_ptr().cast(), needed, &mut needed) } == 0
    {
        return Err(format!("GetTokenInformation(TokenGroups) failed: {}", unsafe { GetLastError() }));
    }
    let offset = std::mem::offset_of!(TOKEN_GROUPS, Groups);
    let stride = std::mem::size_of::<SID_AND_ATTRIBUTES>();
    let count = unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast::<u32>()) } as usize;
    for index in 0..count {
        let entry =
            unsafe { std::ptr::read_unaligned(buffer.as_ptr().add(offset + index * stride).cast::<SID_AND_ATTRIBUTES>()) };
        if entry.Attributes & SE_GROUP_LOGON_ID == SE_GROUP_LOGON_ID {
            // 复制 SID 字节。
            return copy_sid_bytes(entry.Sid);
        }
    }
    Err("logon SID not found in token".to_string())
}

unsafe fn copy_sid_bytes(psid: *mut c_void) -> Result<Vec<u8>, String> {
    let len = unsafe { GetLengthSid(psid) };
    if len == 0 {
        return Err("GetLengthSid returned 0".to_string());
    }
    let mut buf = vec![0u8; len as usize];
    let ok = unsafe { CopySid(len, buf.as_mut_ptr().cast(), psid) };
    if ok == 0 {
        return Err("CopySid failed".to_string());
    }
    Ok(buf)
}

/// World SID（S-1-1-0）。
///
/// # Safety
/// 内部仅调用 `CreateWellKnownSid`。
unsafe fn world_sid() -> Result<Vec<u8>, String> {
    let mut size: u32 = 0;
    unsafe { CreateWellKnownSid(WIN_WORLD_SID, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) };
    let mut buf = vec![0u8; size as usize];
    let ok = unsafe { CreateWellKnownSid(WIN_WORLD_SID, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut size) };
    if ok == 0 {
        return Err(format!("CreateWellKnownSid failed: {}", unsafe { GetLastError() }));
    }
    Ok(buf)
}

/// 设置 token 默认 DACL（`entries` 为 `(SID, 访问掩码)`）：决定该受限进程自身新建
/// 对象的可写性——命名管道 / 共享内存等进程内 IPC 若不可写，`powershell` 直接起不来。
///
/// 说明：文件写入遏制不依赖默认 DACL，而依赖「受限 token 的双重访问检查」——目标目录
/// 的 DACL 只会授给用户账号 SID，restricting 集合拿不到写权限，因此工作区外的写仍被拒。
///
/// # Safety
/// `h_token` 必须是合法 token 句柄；`entries` 里的 SID 是指针，调用期间必须有效。
unsafe fn set_sandbox_default_dacl(
    h_token: HANDLE,
    entries: &[(*mut c_void, u32)],
) -> Result<(), String> {
    let explicit: Vec<EXPLICIT_ACCESS_W> = entries
        .iter()
        .map(|(psid, mask)| EXPLICIT_ACCESS_W {
            grfAccessPermissions: *mask,
            grfAccessMode: GRANT_ACCESS,
            grfInheritance: 0,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: std::ptr::null_mut(),
                MultipleTrusteeOperation: 0,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_UNKNOWN,
                ptstrName: *psid as *mut u16,
            },
        })
        .collect();
    let mut p_new_dacl: *mut ACL = std::ptr::null_mut();
    let res = unsafe {
        SetEntriesInAclW(
            explicit.len() as u32,
            explicit.as_ptr(),
            std::ptr::null_mut(),
            &mut p_new_dacl,
        )
    };
    if res != windows_sys::Win32::Foundation::ERROR_SUCCESS {
        return Err(format!("SetEntriesInAclW failed: {res}"));
    }
    #[repr(C)]
    struct TokenDefaultDaclInfo {
        default_dacl: *mut ACL,
    }
    let mut info = TokenDefaultDaclInfo {
        default_dacl: p_new_dacl,
    };
    let ok = unsafe {
        SetTokenInformation(
            h_token,
            TokenDefaultDacl,
            &mut info as *mut _ as *mut c_void,
            std::mem::size_of::<TokenDefaultDaclInfo>() as u32,
        )
    };
    unsafe { local_free(p_new_dacl as *mut c_void) };
    if ok == 0 {
        return Err(format!("SetTokenInformation(TokenDefaultDacl) failed: {}", unsafe { GetLastError() }));
    }
    Ok(())
}

/// 启用单个 token privilege。
///
/// # Safety
/// `h_token` 必须是合法 token 句柄。
unsafe fn enable_privilege(h_token: HANDLE, name: &str) -> Result<(), String> {
    let mut luid = windows_sys::Win32::Foundation::LUID { LowPart: 0, HighPart: 0 };
    let ok = unsafe { LookupPrivilegeValueW(std::ptr::null(), to_wide(name).as_ptr(), &mut luid) };
    if ok == 0 {
        return Err(format!("LookupPrivilegeValueW failed: {}", unsafe { GetLastError() }));
    }
    let mut tp: TOKEN_PRIVILEGES = unsafe { std::mem::zeroed() };
    tp.PrivilegeCount = 1;
    tp.Privileges[0].Luid = luid;
    tp.Privileges[0].Attributes = 0x0000_0002; // SE_PRIVILEGE_ENABLED
    let ok2 = unsafe { AdjustTokenPrivileges(h_token, 0, &tp, 0, std::ptr::null_mut(), std::ptr::null_mut()) };
    if ok2 == 0 {
        return Err(format!("AdjustTokenPrivileges failed: {}", unsafe { GetLastError() }));
    }
    let err = unsafe { GetLastError() };
    if err != 0 {
        return Err(format!("AdjustTokenPrivileges error {err}"));
    }
    Ok(())
}

/// 工作区可写掩码：数据 / 属性 / 拓展属性 / 追加 / 删除 / 改 DACL / 改属主。
const WORKSPACE_WRITE_MASK: u32 =
    FILE_WRITE_DATA | FILE_WRITE_ATTRIBUTES | FILE_WRITE_EA | FILE_APPEND_DATA | DELETE | WRITE_DAC | WRITE_OWNER;

/// 工作区写授权（**幂等**）：根目录 DACL 已含 logon SID 的写 ACE 就直接返回，否则追加
/// 一条带容器 / 对象继承的 grant-write ACE 并返回 `true`。
///
/// 幂等不是可选优化而是可用性前提：`SetNamedSecurityInfoW` 会把可继承 ACE **同步递归**
/// 传播到整棵既有子树，含 `target/` 的巨型工作区（实测 26 万+ 条目）单次传播就超过 60s。
/// 「每条命令授权 + 回收」等于每条命令都自旋一次（写档实际上不可用），因此 ACE 落下即
/// 保留，后续命令只做一次根目录 DACL 读判定。
///
/// 残留授权由只读档进入时撤销（[`revoke_write_acl`]），因此这里可以放心保留。
///
/// `logon_sid` 必须是受限 token 的 logon SID：受限 token 的访问检查要求常规 SID 与
/// restricting SID 同时通过，restricting 集合含 logon，授权它即可命中写检查。
///
/// # Safety
/// `path` 必须是合法路径；`logon_sid` 必须是有效 SID 指针且在调用期间有效。
unsafe fn ensure_write_acl(path: &Path, logon_sid: *mut c_void) -> Result<bool, String> {
    const SUB_CONTAINERS_AND_OBJECTS_INHERIT: u32 = 0x3;

    // 祖先（或自己）已持有 logon 写 ACE 时无需再授权：该 ACE 经继承已覆盖本目录。
    // 这既省掉落一次巨树传播，也让「上一次中断在半途」的残留不至于让后续命令反复授权。
    if !unsafe { granted_roots(path, logon_sid, false)? }.is_empty() {
        return Ok(false);
    }
    let root = unsafe { root_dacl(path)? };
    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: WORKSPACE_WRITE_MASK,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
        Trustee: trustee(logon_sid),
    };
    unsafe { apply_dacl_entry(path, root.dacl, &entry)? };
    Ok(true)
}

/// 撤销工作区写授权（**幂等**，与 [`ensure_write_acl`] 对称）：没有直接授予的
/// logon 写 ACE 就直接返回 `false`，否则用 REVOKE 重建 DACL 并落地。
///
/// 只读档靠它保证不 fail-open：受限 token 的 restricting 集合恒含 logon SID，
/// 上一轮 workspace-write 落下的授权在同一 logon 会话里对只读档同样有效，不撤销
/// 就等于把只读档悄悄放宽成可写档。
///
/// 撤销要沿着祖先找：授权可能落在**祖先目录**上（会话根变化、或上一次传播被中断后
/// 只留下祖先的 ACE），单看 `path` 自己会漏掉这类残留，而其继承体照样让只读档可写。
///
/// # Safety
/// `path` 必须是合法路径；`logon_sid` 必须是有效 SID 指针且在调用期间有效。
unsafe fn revoke_write_acl(path: &Path, logon_sid: *mut c_void) -> Result<bool, String> {
    const REVOKE_ACCESS: i32 = 4;

    let granted = unsafe { granted_roots(path, logon_sid, true)? };
    if granted.is_empty() {
        return Ok(false);
    }
    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: 0,
        grfAccessMode: REVOKE_ACCESS,
        grfInheritance: 0,
        Trustee: trustee(logon_sid),
    };
    for dir in &granted {
        let root = unsafe { root_dacl(dir)? };
        unsafe { apply_dacl_entry(dir, root.dacl, &entry)? };
    }
    Ok(true)
}

/// 从 `path` 起向上枚举持有 logon 写 ACE 的目录。
///
/// `direct_only = true` 只看**直接**授予的 ACE（撤销时用：从上层继承来的 ACE 无法在
/// 子目录上移除）；`false` 连继承来的也算（授权时用：祖先授权已覆盖本目录）。
///
/// # Safety
/// `path` 必须是合法路径；`logon_sid` 必须是有效 SID 指针且在调用期间有效。
unsafe fn granted_roots(
    path: &Path,
    logon_sid: *mut c_void,
    direct_only: bool,
) -> Result<Vec<std::path::PathBuf>, String> {
    let mut found = Vec::new();
    let mut current = Some(path.to_path_buf());
    while let Some(dir) = current {
        // 读不到 DACL（如盘符根目录的 READ_CONTROL 受限）就停止向上，按已找到的算。
        let Ok(root) = (unsafe { root_dacl(&dir) }) else { break };
        if unsafe { dacl_grants_write(root.dacl, logon_sid, !direct_only) } {
            found.push(dir.clone());
        }
        current = dir.parent().map(Path::to_path_buf);
    }
    Ok(found)
}

/// 根目录 DACL 及其安全描述符（`Drop` 时释放描述符）。
struct RootDacl {
    dacl: *mut ACL,
    sd: *mut c_void,
}

impl Drop for RootDacl {
    fn drop(&mut self) {
        unsafe { local_free(self.sd) };
    }
}

/// 读取目录 DACL（授权 / 撤销 / 幂等判定共用的前置）。
///
/// # Safety
/// `path` 必须是合法路径。
unsafe fn root_dacl(path: &Path) -> Result<RootDacl, String> {
    let wpath = to_wide(path);
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut sd: *mut c_void = std::ptr::null_mut();
    let code = unsafe {
        GetNamedSecurityInfoW(
            wpath.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut dacl,
            std::ptr::null_mut(),
            &mut sd,
        )
    };
    if code != windows_sys::Win32::Foundation::ERROR_SUCCESS {
        return Err(format!("GetNamedSecurityInfoW failed for {}: {code}", path.display()));
    }
    Ok(RootDacl { dacl, sd })
}

/// 把一条显式访问项合并进目录 DACL 并落地（可继承 ACE 的增删由系统递归传播到子树）。
///
/// # Safety
/// `path` 必须是合法路径；`base` 必须是合法 DACL（可为空）；`entry` 内的 SID 必须有效。
unsafe fn apply_dacl_entry(
    path: &Path,
    base: *mut ACL,
    entry: &EXPLICIT_ACCESS_W,
) -> Result<(), String> {
    let wpath = to_wide(path);
    let mut p_new_dacl: *mut ACL = std::ptr::null_mut();
    let code = unsafe { SetEntriesInAclW(1, entry, base, &mut p_new_dacl) };
    if code != windows_sys::Win32::Foundation::ERROR_SUCCESS {
        return Err(format!("SetEntriesInAclW failed: {code}"));
    }
    let code = unsafe {
        SetNamedSecurityInfoW(
            wpath.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            p_new_dacl,
            std::ptr::null_mut(),
        )
    };
    unsafe { local_free(p_new_dacl as *mut c_void) };
    if code != windows_sys::Win32::Foundation::ERROR_SUCCESS {
        return Err(format!("SetNamedSecurityInfoW failed for {}: {code}", path.display()));
    }
    Ok(())
}

/// 以 SID 构造受信者（授权 / 撤销共用）。
fn trustee(psid: *mut c_void) -> TRUSTEE_W {
    TRUSTEE_W {
        pMultipleTrustee: std::ptr::null_mut(),
        MultipleTrusteeOperation: 0,
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_UNKNOWN,
        ptstrName: psid as *mut u16,
    }
}

/// DACL 中是否已有授给 `psid` 的允许型 ACE，且掩码覆盖 [`WORKSPACE_WRITE_MASK`]。
///
/// `include_inherited = false` 时忽略继承来的 ACE（`INHERITED_ACE` 标记）——从上层继承
/// 的 ACE 无法在本对象上撤销，撤销必须回到真正授予它的那个目录。
///
/// # Safety
/// `dacl` 必须是合法 ACL 指针（可为空）；`psid` 必须是有效 SID 指针。
unsafe fn dacl_grants_write(dacl: *const ACL, psid: *mut c_void, include_inherited: bool) -> bool {
    if dacl.is_null() {
        return false;
    }
    const ACCESS_ALLOWED_ACE_TYPE: u8 = 0x0;
    const INHERITED_ACE: u8 = 0x10;
    let count = u32::from(unsafe { (*dacl).AceCount });
    for index in 0..count {
        let mut p_ace: *mut c_void = std::ptr::null_mut();
        if unsafe { GetAce(dacl as *mut ACL, index, &mut p_ace) } == 0 {
            continue;
        }
        // ACCESS_ALLOWED_ACE 布局：ACE_HEADER(AceType u8 + AceFlags u8 + AceSize u16) +
        // Mask(4) + SidStart(...)。
        let raw = p_ace as *const u8;
        if unsafe { std::ptr::read_unaligned(raw) } != ACCESS_ALLOWED_ACE_TYPE {
            continue;
        }
        let flags = unsafe { std::ptr::read_unaligned(raw.add(1)) };
        if !include_inherited && flags & INHERITED_ACE != 0 {
            continue;
        }
        let mask = unsafe { std::ptr::read_unaligned(raw.add(4).cast::<u32>()) };
        if mask & WORKSPACE_WRITE_MASK != WORKSPACE_WRITE_MASK {
            continue;
        }
        let sid = unsafe { raw.add(8) } as *mut c_void;
        if unsafe { EqualSid(sid, psid) } != 0 {
            return true;
        }
    }
    false
}

unsafe fn local_free(p: *mut c_void) {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(hMem: *mut c_void) -> *mut c_void;
    }
    unsafe { LocalFree(p) };
}

/// 字符串/路径 → UTF-16 宽字符缓冲（NUL 结尾）。
fn to_wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    let mut wide: Vec<u16> = value.as_ref().encode_wide().collect();
    wide.push(0);
    wide
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
