//! `runtime.json` 负载元数据解析与校验。
//!
//! 纯函数：读取并规范化已知构建元数据，不重写源文件。校验字段类型、平台/架构
//! 白名单、版本格式、发行版本映射，并对重复的规范化发行名报错。

use std::collections::BTreeMap;

use regex::Regex;
use serde_json::Value;

use super::WorkspaceDepsError;

/// 版本正则：`1.2.3` 或带预发布/构建号（`1.2.3-beta.1+build`）。
const VERSION_RE: &str = r"^\d+\.\d+\.\d+(?:[-+][\w.-]+)?$";
/// 发行名正则：字母数字开头 + `[A-Za-z0-9._-]*`。
const DIST_NAME_RE: &str = r"^[A-Za-z0-9][A-Za-z0-9._-]*$";
/// 发行版本正则：数字开头 + `[\w.!+-]*`。
const DIST_VERSION_RE: &str = r"^\d[\w.!+-]*$";
/// 负载摘要正则：64 位小写十六进制。
const DIGEST_RE: &str = r"^[a-f0-9]{64}$";
/// 合法平台。
const PLATFORMS: &[&str] = &["win32", "darwin", "linux"];
/// 合法架构。
const ARCHS: &[&str] = &["x64", "arm64"];

/// 编译一次正则并匹配（模式为编译期常量，编译必然成功，失败视为不匹配）。
fn matches(pattern: &str, text: &str) -> bool {
    Regex::new(pattern).is_ok_and(|re| re.is_match(text))
}

/// 规范化后的构建元数据（与用户安装的包无关）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryRuntimeManifest {
    pub desktop_version: String,
    pub platform: String,
    pub arch: String,
    /// 锁定负载标识（仅负载哈希化前的安装缺失）。
    pub payload_digest: Option<String>,
    /// 捆绑 Python 解释器版本。
    pub python: String,
    /// 负载未携带 Node.js 时缺失。
    pub node: Option<String>,
    /// 捆绑 pnpm 版本；需要捆绑 Node.js 可执行文件。
    pub pnpm: Option<String>,
    /// 记录的 Python 发行版本映射（仅构建期锁定，不含用户后装）。
    pub python_packages: BTreeMap<String, String>,
}

fn is_version(value: &Value) -> bool {
    value.as_str().is_some_and(|s| matches(VERSION_RE, s))
}

/// 校验并规范化 `runtime.json` 内容。
///
/// 仅支持扁平格式（`desktopVersion`/`platform`/`arch`/`python`/`node`/`pnpm`/
/// `pythonPackages`）。legacy `components` 字段为旧磁盘格式，不落地，
/// 遇 `components` 字段直接按非法元数据拒绝（保持严格）。
pub fn parse_primary_runtime(value: &Value) -> Result<PrimaryRuntimeManifest, WorkspaceDepsError> {
    let obj = value
        .as_object()
        .ok_or_else(|| WorkspaceDepsError::InvalidMetadata("invalid metadata".into()))?;
    if obj.contains_key("components") {
        return Err(WorkspaceDepsError::InvalidMetadata(
            "legacy components format not supported".into(),
        ));
    }
    let desktop_version = obj
        .get("desktopVersion")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| WorkspaceDepsError::InvalidMetadata("invalid metadata".into()))?
        .to_string();
    let platform = obj
        .get("platform")
        .and_then(Value::as_str)
        .filter(|s| PLATFORMS.contains(s))
        .ok_or_else(|| WorkspaceDepsError::InvalidMetadata("invalid metadata".into()))?
        .to_string();
    let arch = obj
        .get("arch")
        .and_then(Value::as_str)
        .filter(|s| ARCHS.contains(s))
        .ok_or_else(|| WorkspaceDepsError::InvalidMetadata("invalid metadata".into()))?
        .to_string();
    let python = obj
        .get("python")
        .filter(|v| is_version(v))
        .and_then(Value::as_str)
        .ok_or_else(|| WorkspaceDepsError::InvalidMetadata("invalid metadata".into()))?
        .to_string();
    // node：可选；存在时须为合法版本。
    let node = obj.get("node").map(|v| {
        v.as_str()
            .filter(|s| is_version(v) && !s.is_empty())
            .ok_or_else(|| WorkspaceDepsError::InvalidMetadata("invalid metadata".into()))
            .map(str::to_string)
    });
    // pnpm：可选；存在时须为合法版本且 node 必须存在（依赖约束）。
    let pnpm = obj.get("pnpm").map(|v| {
        v.as_str()
            .filter(|s| is_version(v) && !s.is_empty())
            .ok_or_else(|| WorkspaceDepsError::InvalidMetadata("invalid metadata".into()))
            .map(str::to_string)
    });
    if obj.contains_key("pnpm") && !obj.contains_key("node") {
        return Err(WorkspaceDepsError::InvalidMetadata(
            "pnpm requires node".into(),
        ));
    }
    let node = node.transpose()?;
    let pnpm = pnpm.transpose()?;
    let payload_digest = obj.get("payloadDigest").map(|v| {
        v.as_str()
            .filter(|s| matches(DIGEST_RE, s))
            .ok_or_else(|| WorkspaceDepsError::InvalidMetadata("invalid metadata".into()))
            .map(str::to_string)
    });
    let payload_digest = payload_digest.transpose()?;
    let raw_map = obj
        .get("pythonPackages")
        .and_then(Value::as_object)
        .filter(|m| {
            m.iter().all(|(name, version)| {
                matches(DIST_NAME_RE, name)
                    && version
                        .as_str()
                        .is_some_and(|s| matches(DIST_VERSION_RE, s))
            })
        })
        .ok_or_else(|| WorkspaceDepsError::InvalidMetadata("invalid metadata".into()))?;
    // 规范化发行名（小写 + 连字符），重复（如 `numpy` 与 `NumPy`）→ 非法。
    let mut python_packages = BTreeMap::new();
    for (name, version) in raw_map {
        let normalized = name.to_lowercase().replace(['-', '_', '.'], "-");
        let version = version.as_str().unwrap_or_default().to_string();
        if python_packages.insert(normalized, version).is_some() {
            return Err(WorkspaceDepsError::InvalidMetadata(
                "duplicate normalized distribution name".into(),
            ));
        }
    }
    Ok(PrimaryRuntimeManifest {
        desktop_version,
        platform,
        arch,
        payload_digest,
        python,
        node,
        pnpm,
        python_packages,
    })
}

/// 读取并规范化 `runtime.json`（不重写源文件）。
pub fn read_primary_runtime(root: &std::path::Path) -> Result<PrimaryRuntimeManifest, WorkspaceDepsError> {
    let raw = std::fs::read_to_string(root.join("runtime.json")).map_err(|e| {
        WorkspaceDepsError::Io(std::io::Error::new(
            e.kind(),
            format!("failed to read {}: {e}", root.join("runtime.json").display()),
        ))
    })?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|e| WorkspaceDepsError::InvalidMetadata(format!("invalid metadata: {e}")))?;
    parse_primary_runtime(&value)
}
