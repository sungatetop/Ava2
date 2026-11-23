//! 文件访问沙箱：读类放行 + 写类 root 遏制 + 路径防逃逸 + 受控 fs 原语。
//!
//! 独立安全基础设施（安全模型第三层，见 docs/design/onion-tools-design.md §4）：
//! 文件类工具不直接触碰 `tokio::fs`，全部文件系统访问必须经 Sandbox 的受控原语。
//! 策略层（模式准入 / 人工审批）决定「要不要允许这次调用」；沙箱是隔离层，
//! 技术保证「被允许的操作也无法越界」——策略失误不影响隔离保证。
//!
//! 权限语义（fs-sandbox 模型）：**遏制只落在写类
//! mutation（write / edit_text / insert_line），读类（read_text / read_image /
//! read / stat / list）与 glob/grep 的显式 path 参数在所有模式下放行**——
//! `WorkingDirectoryService` 的注释原文是 "a resolution default, NOT a containment
//! boundary"。写类**可写根 = 权限根 + 系统临时目录**（`writableRoots =
//! [workspaceRoot, tmpdir()]`）。数据根内的文件（shell 溢出日志、系统记忆等）由此
//! 可读，系统临时目录可写；读取会记录观测门禁（read-before-write），越出可写根的
//! 写入仍被拒绝。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::session_cwd::SessionCwd;
use crate::types::ContentBlock;

/// 文件访问沙箱：所有路径输入都解析为沙箱内的绝对路径后才能触碰文件系统。
///
/// 解析基准是会话的**活动目录**（`SessionCwd::current`），权限边界是**权限根**
/// （`SessionCwd::root`）——二者分离：相对路径基于活动目录，权限根不变。
pub struct Sandbox {
    cwd: Arc<SessionCwd>,
}

impl Sandbox {
    /// 以独立 `SessionCwd` 构造（单会话 / 测试场景；生产装配应共享同一实例）。
    pub fn new(root: PathBuf) -> Self {
        Self::with_cwd(Arc::new(SessionCwd::new(root)))
    }

    /// 以共享的会话目录句柄构造（生产装配路径）。
    pub fn with_cwd(cwd: Arc<SessionCwd>) -> Self {
        Self { cwd }
    }

    /// 权限根（恒定）。
    pub fn root(&self) -> &Path {
        self.cwd.root()
    }

    /// 当前活动目录（相对路径的解析基准）。
    pub fn current(&self) -> Arc<PathBuf> {
        self.cwd.current()
    }

    /// 路径 → 可写根内绝对路径：相对路径基于活动目录，绝对路径直接使用；
    /// 归一化后越出**可写根**（权限根 + 系统临时目录）才拒绝（根内绝对路径与
    /// `..` 均合法）。
    ///
    /// **写类路径专用**——写类原语（write / edit_text / insert_line）必须走此方法
    /// 落可写根遏制；读类请用 [`Sandbox::resolve_read`]。
    pub fn resolve(&self, input: &str) -> Result<PathBuf, String> {
        self.cwd.resolve(input)
    }

    /// 读类路径解析：仅词法归一化，**不做权限根校验**（读放行语义）。
    /// 读类原语与 glob/grep 的显式 path 参数走此方法。
    pub fn resolve_read(&self, input: &str) -> Result<PathBuf, String> {
        self.cwd.resolve_read(input)
    }

    /// 受控读原始字节：观测门禁计算内容版本凭证用（工具层不直接触碰 `tokio::fs`）。
    pub async fn read_bytes(&self, input: &str) -> Result<Vec<u8>, String> {
        let path = self.resolve_read(input)?;
        tokio::fs::read(&path)
            .await
            .map_err(|error| format!("read {} failed: {error}", path.display()))
    }

    /// 受控读文本：仅返回文本内容；图片文件被拒绝并提示改用 `read_image`
    /// （`read` 与 `read_image` 分离：schema 各自单一职责，模型选型明确）。
    pub async fn read_text(&self, input: &str) -> Result<String, String> {
        let path = self.resolve_read(input)?;
        if is_image(&path) {
            return Err(format!(
                "{} is an image file; use read_image instead",
                path.display()
            ));
        }
        let data = tokio::fs::read(&path)
            .await
            .map_err(|error| format!("read {} failed: {error}", path.display()))?;
        Ok(String::from_utf8_lossy(&data).to_string())
    }

    /// 受控 stat：解析路径并返回元数据（用于「存在 + 类型」校验，如 present 工具）。
    /// 不存在 / 越界均 Err。仅做只读元数据查询，不读取内容。
    pub async fn stat(&self, input: &str) -> Result<std::fs::Metadata, String> {
        let path = self.resolve_read(input)?;
        tokio::fs::metadata(&path)
            .await
            .map_err(|error| format!("stat {} failed: {error}", path.display()))
    }

    /// 受控读图片：仅接受图片扩展名，输出 data URL 图片块（多模态）。
    pub async fn read_image(&self, input: &str) -> Result<ContentBlock, String> {
        let path = self.resolve_read(input)?;
        if !is_image(&path) {
            return Err(format!(
                "{} is not an image file; use read instead",
                path.display()
            ));
        }
        let data = tokio::fs::read(&path)
            .await
            .map_err(|error| format!("read {} failed: {error}", path.display()))?;
        let mime = match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase) {
            Some(ext) if ext == "png" => "image/png",
            Some(ext) if ext == "gif" => "image/gif",
            Some(ext) if ext == "webp" => "image/webp",
            _ => "image/jpeg",
        };
        Ok(ContentBlock::Image { image_url: format!("data:{mime};base64,{}", base64_encode(&data)) })
    }

    /// 受控改写（`edit` 语义）：唯一 literal replace。
    ///
    /// `stale_token` 为观测门禁给出的过期凭证（Some = 观测版本与当前内容不一致，
    /// `FS_STALE_VERSION` fail-closed）。None 表示无条件改写（无观测策略时）。
    pub async fn edit_text(
        &self,
        input: &str,
        old: &str,
        new: &str,
        replace_all: bool,
        stale_token: Option<&str>,
    ) -> Result<String, String> {
        let path = self.resolve(input)?;
        if stale_token.is_some() {
            return Err(format!(
                "cannot modify \"{input}\": file changed since it was read (FS_STALE_VERSION) — \
                 read the file again, then retry"
            ));
        }
        if old.is_empty() {
            return Err("old_string must be a non-empty string".to_string());
        }
        if old == new {
            return Err("old_string and new_string must differ".to_string());
        }
        let before = tokio::fs::read_to_string(&path)
            .await
            .map_err(|error| format!("read {} failed: {error}", path.display()))?;
        let occurrences = before.matches(old).count();
        if occurrences == 0 {
            return Err(format!("old_string not found in {input}"));
        }
        if !replace_all && occurrences > 1 {
            return Err(format!(
                "old_string is not unique in {input} ({occurrences} matches) — provide more context \
                 or set replace_all"
            ));
        }
        let after = if replace_all {
            before.replace(old, new)
        } else {
            before.replacen(old, new, 1)
        };
        tokio::fs::write(&path, after.as_bytes())
            .await
            .map_err(|error| format!("write {} failed: {error}", path.display()))?;
        Ok(if replace_all {
            format!("The file {input} has been updated. All occurrences were successfully replaced.")
        } else {
            format!("The file {input} has been updated successfully.")
        })
    }

    /// 受控按行插入（`str_replace_editor` 的 `insert` 动作）：
    /// `insert_line` 为 1 基行号，新行插入到该行之后；负数或越界表示追加到末尾
    /// （`[start_line, -1]` 约定）。
    pub async fn insert_line(&self, input: &str, insert_line: i64, text: &str) -> Result<String, String> {
        let path = self.resolve(input)?;
        let before = tokio::fs::read_to_string(&path)
            .await
            .map_err(|error| format!("read {} failed: {error}", path.display()))?;
        let mut lines: Vec<String> = before.lines().map(str::to_string).collect();
        let position = if insert_line < 0 || insert_line as usize >= lines.len() {
            lines.len()
        } else {
            insert_line as usize
        };
        lines.insert(position, text.to_string());
        let after = lines.join("\n");
        tokio::fs::write(&path, after.as_bytes())
            .await
            .map_err(|error| format!("write {} failed: {error}", path.display()))?;
        Ok(format!("inserted 1 line at line {} of {input}", position + 1))
    }

    /// 受控读（自动分派文本/图片）——供不关心分派的内部调用使用；
    /// 模型可见工具一律走 `read_text` / `read_image` 分离路径。
    pub async fn read(&self, input: &str) -> Result<Vec<ContentBlock>, String> {
        let path = self.resolve_read(input)?;
        if is_image(&path) {
            return Ok(vec![self.read_image(input).await?]);
        }
        Ok(vec![ContentBlock::Text { text: self.read_text(input).await? }])
    }

    /// 受控写：文本内容写入沙箱内文件（自动创建父目录）；返回成功消息。
    pub async fn write(&self, input: &str, content: &str) -> Result<String, String> {
        let path = self.resolve(input)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| format!("create {} failed: {error}", parent.display()))?;
        }
        let bytes = content.as_bytes();
        tokio::fs::write(&path, bytes)
            .await
            .map_err(|error| format!("write {} failed: {error}", path.display()))?;
        Ok(format!("wrote {} bytes to {}", bytes.len(), path.display()))
    }

    /// 受控列目录：沙箱内条目一行一个（字典序）。
    pub async fn list(&self, input: &str) -> Result<String, String> {
        let path = self.resolve_read(input)?;
        let mut reader = tokio::fs::read_dir(&path)
            .await
            .map_err(|error| format!("list {} failed: {error}", path.display()))?;
        let mut lines = Vec::new();
        loop {
            match reader.next_entry().await {
                Ok(Some(entry)) => lines.push(entry.file_name().to_string_lossy().to_string()),
                Ok(None) => break,
                Err(error) => return Err(format!("list {} failed: {error}", path.display())),
            }
        }
        lines.sort();
        Ok(lines.join("\n"))
    }

    /// 受控递归遍历：枚举**活动目录**内全部文件（不含目录）的绝对路径，
    /// 按修改时间升序（`glob --sort=modified`）。供 `glob` / `grep`
    /// 发现工具使用：路径经 root 限定防逃逸；符号链接不跟随（避免越界循环）。
    /// 不可访问的条目跳过而非整体失败（与 ripgrep 对权限受限路径的容忍一致）。
    pub async fn walk(&self) -> Result<Vec<std::path::PathBuf>, String> {
        let mut files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
        let base = self.current();
        self.walk_dir(&base, &mut files).await?;
        // 修改时间升序；兜底路径字典序保证确定性。
        files.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        Ok(files.into_iter().map(|(path, _)| path).collect())
    }

    /// 递归枚举任意目录（读类原语）：glob/grep 的显式 `path` 参数可指向任意
    /// 可读目录（检索工具的 path 语义——path 不经 root 遏制）。
    /// 剪枝与排序规则与 [`Sandbox::walk`] 完全一致；目录不可访问时返回空集。
    pub async fn walk_under(&self, dir: &Path) -> Result<Vec<std::path::PathBuf>, String> {
        let mut files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
        self.walk_dir(dir, &mut files).await?;
        files.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        Ok(files.into_iter().map(|(path, _)| path).collect())
    }

    /// 递归遍历单个目录（不跟随符号链接；不可访问条目跳过）。
    async fn walk_dir(
        &self,
        dir: &Path,
        out: &mut Vec<(PathBuf, std::time::SystemTime)>,
    ) -> Result<(), String> {
        let mut reader = match tokio::fs::read_dir(dir).await {
            Ok(reader) => reader,
            Err(_) => return Ok(()),
        };
        loop {
            let entry = match reader.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(_) => return Ok(()),
            };
            let ft = match entry.file_type().await {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let path = entry.path();
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                // 固定剪枝重目录（构建产物 / 依赖 / VCS 元数据）：这些目录文件量
                // 巨大且与代码检索无关，全量遍历会让 glob/grep 在仓库根上阻塞分钟级
                // （对齐 CC 检索工具的默认 ignore 行为）。
                if is_pruned_dir_name(&entry.file_name().to_string_lossy()) {
                    continue;
                }
                Box::pin(self.walk_dir(&path, out)).await?;
            } else if ft.is_file() {
                let mtime = entry
                    .metadata()
                    .await
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                out.push((path, mtime));
            }
        }
        Ok(())
    }
}

/// walk 剪枝的目录名：Rust 构建产物、Node 依赖、VCS 元数据。
fn is_pruned_dir_name(name: &str) -> bool {
    matches!(name, "target" | "node_modules" | ".git")
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp"))
        .unwrap_or(false)
}

/// 手写 base64（无外部依赖）：图片块编码为 data URL。
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod tests;
