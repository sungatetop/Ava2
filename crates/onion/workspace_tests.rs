//! `workspace` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 覆盖：发现（global → root → ancestor chain）、FsVersion 指纹、
//! dedupe、render_baseline（budget 递减 + omitted）、render_delta、reconcile。

use std::fs;
use std::path::PathBuf;

use super::*;

/// 临时目录（测试结束清理；避免污染真实数据目录）。
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("onion_workspace_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// 在目录下写 AGENTS.md（父目录自动创建）。
fn write_agent_file(dir: &PathBuf, name: &str, body: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    path
}

#[test]
fn discover_finds_project_root_and_ancestors() {
    let tmp = TempDir::new("ancestors");
    let root = tmp.path.join("repo");
    let sub = root.join("a").join("b");
    // .git marker：find_project_root 依赖它定位项目根。
    fs::create_dir_all(root.join(".git")).unwrap();
    write_agent_file(&root, "AGENTS.md", "root rules");
    write_agent_file(&sub, "AGENTS.md", "sub rules");
    write_agent_file(&sub, "AGENTS.local.md", "local rules");
    let ws = Workspace::discover(&sub, 65536).unwrap();
    let paths: Vec<String> = ws
        .files
        .iter()
        .map(|f| f.path.display().to_string())
        .collect();
    // broad-to-specific：root 在前，sub 在后。
    assert_eq!(paths.len(), 3);
    assert!(paths[0].contains("repo\\AGENTS.md") || paths[0].contains("repo/AGENTS.md"));
    assert!(paths[1].contains("a\\b\\AGENTS.md") || paths[1].contains("a/b/AGENTS.md"));
    assert!(paths[2].contains("AGENTS.local.md"));
}

#[test]
fn fs_version_fingerprint_is_stable_within_same_file() {
    let tmp = TempDir::new("fingerprint");
    let file = write_agent_file(&tmp.path, "AGENTS.md", "stable content");
    let v1 = fs_version(&file).unwrap();
    let v2 = fs_version(&file).unwrap();
    assert_eq!(v1, v2);
    assert_eq!(v1.fingerprint(), v2.fingerprint());
}

#[test]
fn dedupe_collapses_identical_content_keeping_first() {
    let content = "same rules here";
    let first = InstructionFile {
        path: PathBuf::from("/outer/AGENTS.md"),
        content: content.to_string(),
        version: FsVersion { dev: 0, ino: 0, size: content.len() as u64, mtime_ns: 1, ctime_ns: 1 },
    };
    let second = InstructionFile {
        path: PathBuf::from("/inner/AGENTS.md"),
        content: content.to_string(),
        version: FsVersion { dev: 0, ino: 0, size: content.len() as u64, mtime_ns: 2, ctime_ns: 2 },
    };
    let deduped = dedupe(vec![first, second]);
    assert_eq!(deduped.len(), 1);
    assert_eq!(deduped[0].path.display().to_string(), "/outer/AGENTS.md");
}

#[test]
fn render_baseline_respects_budget_and_reports_omissions() {
    let files = vec![
        InstructionFile {
            path: PathBuf::from("/a/AGENTS.md"),
            content: "a-rules".to_string(),
            version: FsVersion { dev: 0, ino: 0, size: 7, mtime_ns: 0, ctime_ns: 0 },
        },
        InstructionFile {
            path: PathBuf::from("/b/AGENTS.md"),
            content: "b-rules".to_string(),
            version: FsVersion { dev: 0, ino: 0, size: 7, mtime_ns: 0, ctime_ns: 0 },
        },
    ];
    let ws = Workspace {
        cwd: PathBuf::from("/"),
        global_root: None,
        files,
        max_bytes: 60, // 只够装第一个 section（第二个超预算）
    };
    let rendered = ws.render_baseline();
    assert!(rendered.contains("a-rules"));
    assert!(!rendered.contains("b-rules"));
    assert!(rendered.contains("Workspace instruction budget"));
    assert!(rendered.contains("<system-reminder>"));
}

#[test]
fn render_delta_lists_changes() {
    let ws = Workspace {
        cwd: PathBuf::from("/"),
        global_root: None,
        files: Vec::new(),
        max_bytes: 65536,
    };
    let changes = WorkspaceChanges {
        added: vec!["/new/AGENTS.md".to_string()],
        removed: vec!["/gone/AGENTS.md".to_string()],
        changed: vec!["/root/AGENTS.md".to_string()],
    };
    let rendered = ws.render_delta(&changes);
    assert!(rendered.contains("Added: /new/AGENTS.md"));
    assert!(rendered.contains("Removed: /gone/AGENTS.md"));
    assert!(rendered.contains("Changed: /root/AGENTS.md"));
    assert!(rendered.contains("Re-read affected files"));
}

#[test]
fn reconcile_detects_added_removed_and_changed() {
    let tmp = TempDir::new("reconcile");
    let root = tmp.path.join("repo");
    let sub = root.join("sub");
    // .git marker：find_project_root 依赖它定位项目根。
    fs::create_dir_all(root.join(".git")).unwrap();
    let root_file = write_agent_file(&root, "AGENTS.md", "root rules");
    write_agent_file(&sub, "AGENTS.md", "sub rules");
    let mut ws = Workspace::discover(&sub, 65536).unwrap();
    assert_eq!(ws.files.len(), 2);

    // 变更 root 文件内容。
    fs::write(&root_file, "root rules v2").unwrap();
    // 删除 sub 文件。
    let sub_file = sub.join("AGENTS.md");
    fs::remove_file(&sub_file).unwrap();
    // 新增 global 级文件（同目录）。
    write_agent_file(&sub, "AGENTS.local.md", "fresh local");

    let changes = ws.reconcile().unwrap();
    assert_eq!(changes.added.len(), 1);
    assert!(changes.added[0].contains("AGENTS.local.md"));
    assert_eq!(changes.removed.len(), 1);
    assert!(changes.removed[0].contains("sub\\AGENTS.md") || changes.removed[0].contains("sub/AGENTS.md"));
    assert_eq!(changes.changed.len(), 1);
    assert!(changes.changed[0].contains("repo\\AGENTS.md") || changes.changed[0].contains("repo/AGENTS.md"));
    // 内部状态已刷新：再 reconcile 无变更。
    let changes2 = ws.reconcile().unwrap();
    assert!(changes2.is_empty());
}

#[test]
fn sha1_hex_matches_reference_vector() {
    // SHA-1("abc") = a9993e364706816aba3e25717850c26c9cd0d89d
    assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    // SHA-1("") = da39a3ee5e6b4b0d3255bfef95601890afd80709
    assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
}
