//! 凭据子系统：引用式凭据解析 + 模板展开语义。
//!
//! 引用式 seam：providers.json / mcp.json 等配置只携带**引用**（`${credential:NAME}`
//! 模板），真实值落 `<root>/.credentials` 凭据文件；消费方**按次解析**（每次操作
//! 重新读文件，不跨操作缓存）——改密钥即时生效、无需重启；配置面（describe）只见
//! 掩码预览，永不触达全值。凭据名语法为 POSIX 环境变量名
//!（`[A-Za-z_][A-Za-z0-9_]*`）。
//!
//! 有意收敛的最小子集：
//! - 仅实现引用半区（CredentialRef）；记录半区（CredentialKey / GrantRecord /
//!   authorization grants）在 onion 无消费者（无 OAuth/grant 流程），推迟。
//! - 回退层：凭据文件 → 进程环境；.env 文件栈（file / project-env / user-env）
//!   暂无对应机制。
//! - `credentials/reference-updated` 事件与文件 watcher 不需要：按次解析已覆盖
//!   外部改动即时生效，onion 亦无进程级事件总线消费者。
//! - 文档格式：onion 配置面统一 JSON（providers.json / mcp.json 同族），
//!   凭据文档携带 version 印记 + 引用表。

use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// 凭据文档版本印记（当前 `version: 1`）。
const CREDENTIALS_DOCUMENT_VERSION: u32 = 1;

/// 凭据名是否为 POSIX 环境变量名。
pub fn is_credential_ref_name(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 值中 `${credential:NAME}` 引用提取；出现非法 `${...}` 片段即报语法错误
/// （fail-loud：配置面写错模板
/// 宁可拒绝也不静默当普通字符串）。
pub fn credential_refs_in(value: &str) -> Result<Vec<String>, String> {
    let mut refs = Vec::new();
    let mut consumed = 0usize;
    while let Some(off) = value[consumed..].find("${") {
        let start = consumed + off;
        let after_open = &value[start + 2..];
        let Some(close) = after_open.find('}') else {
            return Err(format!("值含未闭合的 \"${{\" 片段：{value:?}"));
        };
        let inner = &after_open[..close];
        let Some(name) = inner.strip_prefix("credential:") else {
            return Err(format!(
                "值含非法模板片段 \"$({inner})\"（仅支持 ${{credential:NAME}}）：{value:?}"
            ));
        };
        if !is_credential_ref_name(name) {
            return Err(format!(
                "模板凭据名 \"{name}\" 非法（须为 POSIX 环境变量名）：{value:?}"
            ));
        }
        refs.push(name.to_string());
        consumed = start + 2 + close + 1;
    }
    Ok(refs)
}

/// 模板展开失败。
#[derive(Debug)]
pub enum TemplateError {
    /// 模板语法非法（fail-loud）。
    Syntax(String),
    /// 引用的凭据未配置（操作失败并点名凭据，
    /// 而非静默传空）。
    Missing { name: String },
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(msg) => f.write_str(msg),
            Self::Missing { name } => {
                write!(f, "凭据 {name} 未配置（既不在凭据文件也不在环境）")
            }
        }
    }
}

impl std::error::Error for TemplateError {}

/// 展开值中的 `${credential:NAME}`：每个引用按出现顺序经 `resolver` 解析，
/// 未配置即 [`TemplateError::Missing`]。
pub fn resolve_template(
    value: &str,
    resolver: &dyn Fn(&str) -> Option<String>,
) -> Result<String, TemplateError> {
    let refs = credential_refs_in(value).map_err(TemplateError::Syntax)?;
    if refs.is_empty() {
        return Ok(value.to_string());
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    for name in &refs {
        let token = format!("${{credential:{name}}}");
        let Some((before, after)) = rest.split_once(&token) else {
            // 不变量由 credential_refs_in 保证；防御分支按未配置处理（fail-closed）。
            return Err(TemplateError::Missing { name: name.clone() });
        };
        out.push_str(before);
        let Some(resolved) = resolver(name) else {
            return Err(TemplateError::Missing { name: name.clone() });
        };
        out.push_str(&resolved);
        rest = after;
    }
    out.push_str(rest);
    Ok(out)
}

/// 一次解析的结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedCredential {
    /// 非空密钥值。
    pub value: String,
    /// 供值层。
    pub source: CredentialSource,
}

/// 解析来源层（两档：凭据文件或进程环境）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialSource {
    /// `<root>/.credentials` 凭据文件。
    File,
    /// 进程环境变量。
    Env,
}

impl CredentialSource {
    /// 来源层标识（`env` / `file`）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Env => "env",
        }
    }
}

/// 配置面事实：永不触达全值。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredentialInfo {
    /// 解析当前是否返回值。
    pub configured: bool,
    /// 当前供值层；未配置时为 None。
    pub source: Option<CredentialSource>,
    /// 凭据文件存储恒可写（无只读 provider）。
    pub writable: bool,
    /// 掩码预览（[`mask_credential`]）；未配置时为 None。
    pub masked: Option<String>,
}

/// 掩码预览（配置面专用）：仅暴露长度与少量首尾字符，
/// 永不触达全值。短值（< 16 字符）全部打点，避免短密钥被碎片拼回。
pub fn mask_credential(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() >= 16 {
        let head: String = chars[..4].iter().collect();
        let tail: String = chars[chars.len() - 4..].iter().collect();
        format!("{head}…{tail}（{} 字符）", chars.len())
    } else {
        format!("••••••••（{} 字符）", chars.len())
    }
}

/// `.credentials` 文档：`{ "version": 1, "refs": { "NAME": "secret" } }`。
/// BTreeMap 保证序列化键序稳定，落盘 diff 可读。
#[derive(Debug, Default, Serialize, Deserialize)]
struct CredentialsDocument {
    version: u32,
    #[serde(default)]
    refs: BTreeMap<String, String>,
}

/// 凭据存储（引用半区）：`<root>/.credentials` JSON 文档 + 进程环境回退。
/// 无缓存无生命周期约束——解析按次重读文件（改密钥即时生效），写路径经
/// 独立锁文件跨进程互斥。
#[derive(Clone, Debug)]
pub struct CredentialStore {
    file: PathBuf,
}

impl CredentialStore {
    /// 以数据根构建（凭据文件 [`crate::paths::credentials_file`]，
    /// 锁文件为同目录 `.credentials.lock`）。
    pub fn new(root: &Path) -> Self {
        Self {
            file: crate::paths::credentials_file(root),
        }
    }

    /// 解析一个引用：凭据文件中的非空值优先，进程环境回退；均缺席 → None。
    /// 每次调用重读文件——不跨操作缓存（按次解析是
    /// 「改密钥无需重启」的机制本体）。
    pub fn resolve(&self, name: &str) -> io::Result<Option<ResolvedCredential>> {
        self.resolve_with(name, &|n| std::env::var(n).ok())
    }

    /// [`resolve`] 的环境注入形态（测试用；生产方传 [`std::env::var`] 包装）。
    pub(crate) fn resolve_with(
        &self,
        name: &str,
        env_lookup: &dyn Fn(&str) -> Option<String>,
    ) -> io::Result<Option<ResolvedCredential>> {
        if !is_credential_ref_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("凭据名 \"{name}\" 非法（须为 POSIX 环境变量名）"),
            ));
        }
        let doc = self.read_document()?;
        if let Some(value) = doc.refs.get(name).filter(|v| !v.is_empty()) {
            return Ok(Some(ResolvedCredential {
                value: value.clone(),
                source: CredentialSource::File,
            }));
        }
        // 空值处处缺席：文件空值不短路，环境空值同样视为缺席。
        Ok(env_lookup(name)
            .filter(|v| !v.is_empty())
            .map(|value| ResolvedCredential {
                value,
                source: CredentialSource::Env,
            }))
    }

    /// 展开模板，引用经本存储解析。读文件失败按「未配置」失败关闭（fail-closed）：
    /// 无法可信读取时绝不把损坏文档当作「无凭据」放行。
    pub fn resolve_template(&self, value: &str) -> Result<String, TemplateError> {
        resolve_template(value, &|name| match self.resolve(name) {
            Ok(Some(resolved)) => Some(resolved.value),
            _ => None,
        })
    }

    /// 持久化存储一个非空值（锁下读改写，跨进程互斥）。空值拒绝（移除用
    /// [`Self::unset`]），写入的值优先于环境回退。
    pub fn set(&self, name: &str, value: &str) -> io::Result<()> {
        if !is_credential_ref_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("凭据名 \"{name}\" 非法（须为 POSIX 环境变量名）"),
            ));
        }
        if value.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("凭据 {name} 的值不能为空（移除请用 unset）"),
            ));
        }
        let _lease = acquire_document_lock(&self.lock_file())?;
        let mut doc = self.read_document()?;
        doc.version = CREDENTIALS_DOCUMENT_VERSION;
        doc.refs.insert(name.to_string(), value.to_string());
        self.write_document(&doc)
    }

    /// 移除一个引用；移除缺席引用是 no-op（不重写文档）。移除后环境回退层
    /// 可能重新生效。
    pub fn unset(&self, name: &str) -> io::Result<()> {
        if !is_credential_ref_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("凭据名 \"{name}\" 非法（须为 POSIX 环境变量名）"),
            ));
        }
        let _lease = acquire_document_lock(&self.lock_file())?;
        let mut doc = self.read_document()?;
        if doc.refs.remove(name).is_none() {
            return Ok(());
        }
        self.write_document(&doc)
    }

    /// 配置面事实：configured / source / writable / masked，永不触达全值。
    pub fn describe(&self, name: &str) -> io::Result<CredentialInfo> {
        Ok(match self.resolve(name)? {
            Some(ResolvedCredential { value, source }) => CredentialInfo {
                configured: true,
                source: Some(source),
                writable: true,
                masked: Some(mask_credential(&value)),
            },
            None => CredentialInfo {
                configured: false,
                source: None,
                writable: true,
                masked: None,
            },
        })
    }

    fn lock_file(&self) -> PathBuf {
        self.file.with_extension("lock")
    }

    fn read_document(&self) -> io::Result<CredentialsDocument> {
        match std::fs::read_to_string(&self.file) {
            Ok(text) => serde_json::from_str(&text).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "凭据文件 {} 损坏（{}）；拒绝当空文档处理",
                        self.file.display(),
                        err
                    ),
                )
            }),
            // 缺失 = 空文档；其余 IO 错误如实上抛。
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(CredentialsDocument::default()),
            Err(err) => Err(err),
        }
    }

    fn write_document(&self, doc: &CredentialsDocument) -> io::Result<()> {
        let text = serde_json::to_string_pretty(doc).map_err(std::io::Error::other)?;
        let tmp = self.file.with_extension("tmp");
        std::fs::write(&tmp, text.as_bytes())?;
        // Windows rename 不覆盖既有目标：锁保护下先移除旧文档，
        // tmp+rename 原子替换；竞态窗口被文档锁排除。
        let _ = std::fs::remove_file(&self.file);
        std::fs::rename(&tmp, &self.file)?;
        restrict_permissions(&self.file);
        Ok(())
    }
}

/// 文档写租约：以独立锁文件保护读改写。
fn acquire_document_lock(lock_path: &Path) -> io::Result<crate::session_lock::SessionWriteLease> {
    crate::session_lock::SessionWriteLease::acquire_at(lock_path).map_err(|err| {
        io::Error::other(format!("凭据写锁 {} 获取失败：{err}", lock_path.display()))
    })
}

/// POSIX 上收紧为 0600（凭据/密钥类文件的安全约定）；
/// Windows 由用户目录 NTFS ACL 继承。
#[cfg(unix)]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) {}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
