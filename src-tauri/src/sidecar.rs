//! onion-server 边车拉起/守护（薄壳 spawn）。
//!
//! 职责：
//! 1. 定位 `onion-server` 二进制（开发态 `<src-tauri>/resources`，打包态 resource_dir）。
//! 2. 以固定端口（默认 9800，与前端默认端点一致）spawn 子进程，注入环境变量：
//!    - `ONION_SERVER_PORT=<port>`
//!    - `ONION_SERVER_CORS_ORIGINS=<app 来源>`（宿主与边车跨源，必须放行，见 `CORS_ORIGINS`）
//!    - 继承父环境（`ONION_HOME` / provider 密钥等穿透）；`ONION_SERVER_WORKSPACE`
//!      不注入，边车以进程 cwd 兜底（开发态 tauri dev 下为 src-tauri）。
//! 3. 输出重定向到 `<ONION_HOME>/logs/onion-server.log`（每次启动滚动为 `.old`）。
//! 4. 退出清理（窗口销毁 / `RunEvent::Exit` 时 kill 子进程树）。
//!
//! 与 ava-app 的差异：onion-server 未提供优雅关闭端点，停止一律走进程树终止。

use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use tauri::AppHandle;
// `Manager` 仅 release 态 `resolve_binary_path` 的 `app.path()` 需要。
#[cfg(not(debug_assertions))]
use tauri::Manager;

/// 边车默认端口（与前端 `client.ts` 的默认端点一致）。
pub const DEFAULT_PORT: u16 = 9800;

/// 允许访问边车的前端来源（AppHost 与边车不同源，浏览器强制 CORS）。
///
/// - `http://localhost:1420` / `http://127.0.0.1:1420`：Vite dev server（web-ui 与 `tauri dev` 共用）
/// - `http://tauri.localhost`：Windows 打包态 webview 来源
/// - `tauri://localhost`：macOS / Linux 打包态 webview 来源
const CORS_ORIGINS: &str =
    "http://localhost:1420,http://127.0.0.1:1420,http://tauri.localhost,tauri://localhost";

/// Windows：为子进程隐藏控制台窗口（对应 `CREATE_NO_WINDOW`）。
///
/// release 态 ava2 是 GUI 子系统进程，缺省 spawn 控制台子进程时 Windows 会为其
/// 新建控制台窗口，且实测会在 conhost 握手阶段死锁。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 边车运行状态（进程句柄 + 端口）。
pub struct Sidecar {
    child: Mutex<Option<Child>>,
    port: u16,
}

impl Sidecar {
    pub fn new(port: u16) -> Self {
        Self {
            child: Mutex::new(None),
            port,
        }
    }

    /// 边车监听端口（前端据此拼基地址）。
    pub fn port(&self) -> u16 {
        self.port
    }

    /// 启动边车。若已运行则直接返回。
    pub fn start(&self, app: &AppHandle) -> Result<(), String> {
        let mut guard = self
            .child
            .lock()
            .map_err(|_| "sidecar lock poisoned".to_string())?;
        if let Some(child) = guard.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Ok(()); // 已在运行
            }
        }

        let binary = normalize_windows_path(resolve_binary_path(app));
        if !binary.exists() {
            return Err(format!(
                "onion-server 边车未找到: {}（请先构建并拷贝到 src-tauri/resources）",
                binary.display()
            ));
        }

        // 日志目录：<ONION_HOME>/logs
        let log_dir = data_home().join("logs");
        fs::create_dir_all(&log_dir).map_err(|e| format!("创建日志目录失败: {e}"))?;
        let log_path = log_dir.join("onion-server.log");
        let old_path = log_dir.join("onion-server.log.old");
        if log_path.exists() {
            let _ = fs::rename(&log_path, &old_path);
        }
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("打开日志文件失败: {e}"))?;
        let log_clone = log_file.try_clone().map_err(|e| e.to_string())?;

        let mut cmd = Command::new(&binary);
        // Windows：release 态 ava2 是 GUI 子系统（无控制台），若不加 CREATE_NO_WINDOW，
        // 控制台子进程（onion-server.exe）会申请新控制台并在 conhost 握手阶段死锁。
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.env("ONION_SERVER_PORT", self.port.to_string());
        // 宿主前端与边车跨源：不放行则浏览器侧 fetch / WS 全部被 CORS 拦截。
        cmd.env("ONION_SERVER_CORS_ORIGINS", CORS_ORIGINS);
        // 安装包内置默认 App（chatapp / file-explorer / file-viewer）：边车首次启动时
        // 种子安装进 <ONION_HOME>/apps。目录缺失（未构建/未打包）时边车静默跳过。
        if let Some(dir) = bundled_apps_dir(app) {
            cmd.env("ONION_BUNDLED_APPS_DIR", dir);
        }
        // 继承父进程环境：ONION_HOME / provider 密钥 / 网络代理等穿透到边车。
        cmd.stdout(Stdio::from(log_file));
        cmd.stderr(Stdio::from(log_clone));

        let child = cmd
            .spawn()
            .map_err(|e| format!("spawn onion-server 失败: {e}"))?;
        *guard = Some(child);

        eprintln!(
            "[sidecar] onion-server 已启动 pid={} port={} log={}",
            guard.as_ref().map(|c| c.id()).unwrap_or(0),
            self.port,
            log_path.display()
        );
        Ok(())
    }

    /// 停止边车：连同子进程树强制终止（onion-server 无优雅关闭端点）。
    pub fn stop(&self) {
        let mut guard = match self.child.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if let Some(mut child) = guard.take() {
            kill_process_tree(child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
        eprintln!("[sidecar] onion-server 已停止");
    }
}

/// 终止指定进程及其全部后代进程（Windows 走 `taskkill /T`）。
///
/// onion-server 会 spawn 工具子进程（shell / 终端 / MCP server），只终止父进程会让
/// 它们成为孤儿并继续占用安装目录内的 exe，导致后续安装器覆盖文件失败。
#[cfg(windows)]
fn kill_process_tree(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(windows))]
fn kill_process_tree(_pid: u32) {}

/// 数据根：`ONION_HOME`（空视为未设置）> `~/.onion`，与 `onion::paths::data_home` 同源。
fn data_home() -> PathBuf {
    if let Ok(value) = std::env::var("ONION_HOME") {
        if !value.trim().is_empty() {
            return PathBuf::from(value.trim());
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(home).join(".onion")
}

/// 定位 onion-server 二进制。
///
/// - 开发态（debug_assertions）：`<src-tauri>/resources/onion-server[.exe]`。
/// - 打包态：`resource_dir()/onion-server[.exe]`（与 tauri.conf.json 的 bundle.resources 一致）。
fn resolve_binary_path(app: &AppHandle) -> PathBuf {
    let exe_name = if cfg!(windows) {
        "onion-server.exe"
    } else {
        "onion-server"
    };

    #[cfg(debug_assertions)]
    {
        let _ = app;
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join(exe_name)
    }

    #[cfg(not(debug_assertions))]
    {
        app.path()
            .resource_dir()
            .unwrap_or_default()
            .join(exe_name)
    }
}

/// 定位内置默认 App 目录（`apps/` 子目录，与边车二进制同资源根）。
///
/// - 开发态（debug_assertions）：`<src-tauri>/resources/apps`。
/// - 打包态：`resource_dir()/apps`（tauri.conf.json 的 bundle.resources 一并打包）。
/// 目录不存在返回 None（dev 未构建 App 产物时正常）。
fn bundled_apps_dir(app: &AppHandle) -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    let root = {
        let _ = app;
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources")
    };

    #[cfg(not(debug_assertions))]
    let root = app.path().resource_dir().unwrap_or_default();

    let dir = normalize_windows_path(root.join("apps"));
    if dir.is_dir() { Some(dir) } else { None }
}

/// 剥离扩展路径前缀（`\\?\C:\...` / `\\?\UNC\server\share`），还原普通 Win32 路径。
fn normalize_windows_path(path: PathBuf) -> PathBuf {
    let s = path.as_os_str().to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

/// 等待边车端口就绪（轮询 TCP 连接，最多约 timeout 秒）。
pub fn wait_ready(port: u16, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    false
}

#[cfg(test)]
#[path = "sidecar_tests.rs"]
mod tests;
