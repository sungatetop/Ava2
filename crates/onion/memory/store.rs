//! `MemoryStore` 门面：保存/读取/删除/列表 + 索引联动。
//!
//! 对齐 ava-harness::memory 设计蓝本：保存记忆是两步：
//! ① 写主题文件；② 更新 `MEMORY.md` 指针。

use std::fs;
use std::path::PathBuf;

use crate::memory::frontmatter::{render_frontmatter, MemoryFrontmatter};
use crate::memory::index::{read_entrypoint, remove_index_line, upsert_index_line};
use crate::memory::paths::{slug_filename, MemoryPaths};
use crate::memory::scan::scan_memory_files;
use crate::memory::types::{MemoryHeader, MemoryType};
use crate::memory::{MemoryError, Result};

/// 一份已加载的记忆文件。
#[derive(Debug, Clone)]
pub struct MemoryFile {
    pub filename: String,
    pub path: PathBuf,
    pub name: String,
    pub description: String,
    pub memory_type: Option<MemoryType>,
    pub body: String,
}

/// 记忆存储门面。
#[derive(Debug, Clone)]
pub struct MemoryStore {
    paths: MemoryPaths,
}

impl MemoryStore {
    pub fn new(paths: MemoryPaths) -> Self {
        Self { paths }
    }

    /// 记忆根目录。
    pub fn root(&self) -> &std::path::Path {
        &self.paths.root
    }

    /// 保存一条记忆：写主题文件 + 更新索引。
    ///
    /// 文件名由 `name` 派生（slug 化防路径穿越）。返回写入的文件路径。
    pub fn save(
        &self,
        name: &str,
        memory_type: MemoryType,
        description: &str,
        body: &str,
    ) -> Result<PathBuf> {
        let filename = format!("{}.md", slug_filename(name));
        let path = self.paths.root.join(&filename);
        // 边界校验：主题文件必须位于记忆根内。
        if !self.paths.is_within(&path) {
            return Err(MemoryError::InvalidPath(format!(
                "memory path escapes root: {}",
                path.display()
            )));
        }
        let fm = MemoryFrontmatter {
            name: name.to_string(),
            description: description.to_string(),
            memory_type: Some(memory_type),
        };
        let content = render_frontmatter(&fm, body);
        fs::create_dir_all(&self.paths.root)?;
        fs::write(&path, content)?;
        // 第二步：更新索引。
        upsert_index_line(&self.paths.root, &filename, name, description)?;
        Ok(path)
    }

    /// 部分更新一条记忆（filename 定位；`Some` 字段覆盖，`None` 保持不变）。
    ///
    /// name 不可改（改名 = [`MemoryStore::delete`] + [`MemoryStore::save`]，
    /// 避免 slug 变更引发旧文件残留）；`MEMORY.md` 索引文件拒绝更新。复用
    /// frontmatter 渲染与索引 upsert，description 变更同步索引行。
    pub fn update(
        &self,
        filename: &str,
        memory_type: Option<MemoryType>,
        description: Option<&str>,
        body: Option<&str>,
    ) -> Result<PathBuf> {
        if filename.eq_ignore_ascii_case(crate::memory::index::ENTRYPOINT_NAME) {
            return Err(MemoryError::InvalidPath(format!(
                "{filename} is the index file and cannot be updated"
            )));
        }
        let existing = self.load(filename)?.ok_or_else(|| {
            MemoryError::InvalidPath(format!("memory not found: {filename}"))
        })?;
        let fm = MemoryFrontmatter {
            name: existing.name,
            description: description.map(str::to_string).unwrap_or(existing.description),
            memory_type: memory_type.or(existing.memory_type),
        };
        let content = render_frontmatter(&fm, body.unwrap_or(&existing.body));
        let path = self.paths.root.join(filename);
        fs::write(&path, content)?;
        upsert_index_line(&self.paths.root, filename, &fm.name, &fm.description)?;
        Ok(path)
    }

    /// 读取一条记忆（按文件名）。
    pub fn load(&self, filename: &str) -> Result<Option<MemoryFile>> {
        let path = self.paths.root.join(filename);
        if !path.exists() || !self.paths.is_within(&path) {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        let parsed = crate::memory::frontmatter::parse_frontmatter(&content);
        Ok(Some(MemoryFile {
            filename: filename.to_string(),
            path,
            name: parsed.fm.name,
            description: parsed.fm.description,
            memory_type: parsed.fm.memory_type,
            // 归一化正文：剔除 frontmatter 分隔后的空行与尾部换行。
            body: parsed.body.trim_matches(['\n', '\r']).to_string(),
        }))
    }

    /// 删除一条记忆（文件 + 索引行）。
    pub fn delete(&self, filename: &str) -> Result<()> {
        let path = self.paths.root.join(filename);
        if path.exists() && self.paths.is_within(&path) {
            fs::remove_file(&path)?;
        }
        remove_index_line(&self.paths.root, filename)
    }

    /// 列出所有记忆头（按 mtime 新→旧）。
    pub fn list(&self) -> Result<Vec<MemoryHeader>> {
        scan_memory_files(&self.paths.root)
    }

    /// 有界索引内容（每次会话加载）。
    pub fn entrypoint(&self) -> Result<String> {
        read_entrypoint(&self.paths.root)
    }

    /// 有界全量清单（filename + type + description + body）。
    ///
    /// 供整合 fork-agent 的输入：单条 body 截断到 `max_body_chars`，总量截断到
    /// `max_total_bytes`（防超大目录撑爆上下文）。
    pub fn manifest(&self, max_body_chars: usize, max_total_bytes: usize) -> Result<String> {
        let mut out = String::new();
        for h in self.list()? {
            if out.len() >= max_total_bytes {
                break;
            }
            let Some(file) = self.load(&h.filename)? else {
                continue;
            };
            let body: String = file.body.trim().chars().take(max_body_chars).collect();
            let ty = file
                .memory_type
                .map(|t| t.as_str().to_string())
                .unwrap_or_else(|| "project".to_string());
            out.push_str(&format!(
                "### {}\ntype: {ty}\ndescription: {}\nbody:\n{body}\n\n",
                file.filename, file.description
            ));
        }
        Ok(out)
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
