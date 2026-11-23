//! office 引擎：定位 LibreOffice `soffice`，经一次性 profile 隔离的 headless 转换执行
//! convert / recalculate / 首页 PNG 渲染；产物落私有输出店（系统临时目录，
//! 在沙箱可写根内、不写会话工作区，模型可经 shell 自行拷贝），按保留期懒清扫。
//!
//! 已知限制：无编程式逐页渲染——
//! `--convert-to png` 仅导出首页/首表，完整文档以 `--convert-to pdf` 获取；
//! recalculate 经同格式 round-trip 实现（LibreOffice 加载 OOXML/ODF 时按
//! 缺省策略重算公式）。

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

/// soffice 执行超时（缺省 60s）。
const CONVERT_TIMEOUT: Duration = Duration::from_secs(60);
/// 转换产物字节上限（有界读输出）。
pub const MAX_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;
/// 源文件字节上限（超限报 `input-too-large`）。
pub const MAX_INPUT_BYTES: u64 = 50 * 1024 * 1024;
/// 私有输出保留期（到期懒清扫）。
const OUTPUT_RETENTION: Duration = Duration::from_secs(24 * 60 * 60);

/// 文档家族。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Text,
    Spreadsheet,
    Presentation,
    Pdf,
}

/// 扩展名 → 家族（大小写不敏感）。
pub fn family_of(extension: &str) -> Option<Family> {
    match extension.to_ascii_lowercase().as_str() {
        "doc" | "docx" | "odt" | "rtf" | "txt" => Some(Family::Text),
        "xls" | "xlsx" | "ods" | "csv" => Some(Family::Spreadsheet),
        "ppt" | "pptx" | "odp" => Some(Family::Presentation),
        "pdf" => Some(Family::Pdf),
        _ => None,
    }
}

/// 家族允许的转换目标（pdf 仅支持首页渲染）。
pub fn conversion_targets(family: Family) -> &'static [&'static str] {
    match family {
        Family::Text => &["pdf", "docx", "odt", "txt"],
        Family::Spreadsheet => &["pdf", "xlsx", "ods", "csv"],
        Family::Presentation => &["pdf", "pptx", "odp"],
        Family::Pdf => &[],
    }
}

/// 引擎错误：`Display` 直接产出错误码前缀（`code: message`），
/// 工具层原样透传给模型。
#[derive(Debug)]
pub enum OfficeError {
    Unavailable,
    InvalidRequest,
    InvalidDocument,
    UnsupportedFormat,
    InvalidOutput,
    OutputTooLarge,
    InputTooLarge,
    Timeout,
    Failed(String),
}

impl fmt::Display for OfficeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => write!(f, "unavailable: LibreOffice (soffice) is not installed"),
            Self::InvalidRequest => write!(f, "invalid-request: missing or malformed arguments"),
            Self::InvalidDocument => write!(f, "invalid-document: input is not a parsable document"),
            Self::UnsupportedFormat => write!(f, "unsupported-format: format not allowed for this input"),
            Self::InvalidOutput => write!(f, "invalid-output: soffice produced no usable output"),
            Self::OutputTooLarge => write!(f, "output-too-large: converted output exceeds the byte limit"),
            Self::InputTooLarge => write!(f, "input-too-large: input file exceeds the byte limit"),
            Self::Timeout => write!(f, "timeout: soffice did not finish in time"),
            Self::Failed(details) => write!(f, "failed: {details}"),
        }
    }
}

/// soffice 引擎句柄（定位结果）。
#[derive(Clone, Debug)]
pub struct Engine {
    binary: PathBuf,
}

impl Engine {
    /// 定位 soffice：`ONION_SOFFICE` 显式指定 → PATH → 常见安装路径。
    pub fn locate() -> Option<Self> {
        if let Ok(path) = std::env::var("ONION_SOFFICE") {
            let candidate = PathBuf::from(path);
            if candidate.is_file() {
                return Some(Self { binary: candidate });
            }
        }
        let name = if cfg!(windows) { "soffice.exe" } else { "soffice" };
        if let Some(paths) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&paths) {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(Self { binary: candidate });
                }
            }
        }
        const CANDIDATES: &[&str] = &[
            r"C:\Program Files\LibreOffice\program\soffice.exe",
            r"C:\Program Files (x86)\LibreOffice\program\soffice.exe",
            "/usr/bin/soffice",
            "/usr/local/bin/soffice",
            "/opt/libreoffice/program/soffice",
            "/Applications/LibreOffice.app/Contents/MacOS/soffice",
        ];
        CANDIDATES
            .iter()
            .map(PathBuf::from)
            .find(|candidate| candidate.is_file())
            .map(|binary| Self { binary })
    }

    /// headless 转换：源字节写入 `run_dir`，经独占 profile 执行
    /// `--convert-to target`，产物落在 `run_dir/out` 下（单一文件）。
    pub async fn convert(
        &self,
        source_bytes: &[u8],
        source_ext: &str,
        target: &str,
        run_dir: &Path,
    ) -> Result<Converted, OfficeError> {
        let source = run_dir.join(format!("source.{source_ext}"));
        let out_dir = run_dir.join("out");
        let profile = run_dir.join("profile");
        tokio::fs::write(&source, source_bytes)
            .await
            .map_err(|error| OfficeError::Failed(format!("write source failed: {error}")))?;
        tokio::fs::create_dir_all(&out_dir)
            .await
            .map_err(|error| OfficeError::Failed(format!("create out dir failed: {error}")))?;
        let mut command = tokio::process::Command::new(&self.binary);
        command
            .arg("--headless")
            .arg("--norestore")
            .arg("--nolockcheck")
            .arg("--convert-to")
            .arg(target)
            .arg("--outdir")
            .arg(&out_dir)
            .arg(format!("-env:UserInstallation={}", file_url(&profile)))
            .arg(&source)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        // soffice 失败常仍返回退出码 0，退出码不可靠——以产物存在性判定。
        let _ = tokio::time::timeout(CONVERT_TIMEOUT, command.output())
            .await
            .map_err(|_| OfficeError::Timeout)?
            .map_err(|error| OfficeError::Failed(format!("soffice spawn failed: {error}")))?;
        let output = first_regular_file(&out_dir)
            .await
            .ok_or(OfficeError::InvalidOutput)?;
        let bytes = tokio::fs::metadata(&output)
            .await
            .map_err(|error| OfficeError::Failed(format!("stat output failed: {error}")))?
            .len();
        if bytes > MAX_OUTPUT_BYTES {
            return Err(OfficeError::OutputTooLarge);
        }
        Ok(Converted { path: output, bytes, format: target.to_string() })
    }
}

/// 一次转换的产物（路径位于私有输出店内）。
#[derive(Debug)]
pub struct Converted {
    pub path: PathBuf,
    pub bytes: u64,
    pub format: String,
}

/// 读回产物字节（工具层不直接触碰 fs，统一经引擎原语）。
pub async fn read_output(converted: &Converted) -> Result<Vec<u8>, OfficeError> {
    tokio::fs::read(&converted.path)
        .await
        .map_err(|error| OfficeError::Failed(format!("read output failed: {error}")))
}

static RUN_COUNTER: AtomicU32 = AtomicU32::new(0);

/// 私有输出店根：系统临时目录（沙箱可写根内、不写会话工作区）。
fn store_root() -> PathBuf {
    std::env::temp_dir().join("onion-office")
}

/// 创建一次转换的 run 目录 `<tmp>/onion-office/<pid>-<计数>/`（先做保留期懒清扫）。
pub fn create_run_dir() -> Result<PathBuf, OfficeError> {
    sweep_stale_outputs();
    let run = store_root().join(format!(
        "{}-{}",
        std::process::id(),
        RUN_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&run)
        .map_err(|error| OfficeError::Failed(format!("create run dir failed: {error}")))?;
    Ok(run)
}

/// 保留期懒清扫：删除超过 [`OUTPUT_RETENTION`] 的 run 目录
/// （每次 office 工具执行前调用，无需启动接线）。
pub fn sweep_stale_outputs() {
    sweep_in(&store_root(), OUTPUT_RETENTION);
}

/// 清扫的根与保留期显式传入（测试可注入临时目录与零保留期）。
fn sweep_in(root: &Path, retention: Duration) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let deadline = SystemTime::now() - retention;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .map(|modified| modified < deadline)
            .unwrap_or(false);
        if stale {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// 列出目录内第一个常规文件（一次转换只有一个产物）。
async fn first_regular_file(dir: &Path) -> Option<PathBuf> {
    let mut entries = tokio::fs::read_dir(dir).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry.path().is_file() {
            return Some(entry.path());
        }
    }
    None
}

/// 路径 → `file:///` URL（soffice `-env:UserInstallation` 要求 URL 形态；
/// Windows 盘符路径需规范化为正斜杠并补前导 `/`）。
fn file_url(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
