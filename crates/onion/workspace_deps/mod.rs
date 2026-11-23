//! `load_workspace_dependencies` 工具：查询打包的 Python / Node.js / pnpm 负载的
//! 绝对路径与发行版本。
//!
//! 语义：
//! - 无参数；返回 `{python, node?, pnpm?, pythonPackages, nodePackages?, pythonDistributions}`；
//! - 负载 = 一个携带 `runtime.json` + `dependencies/` 的目录（`source` 配置），
//!   就地使用（`resolvePrimaryRuntime` 语义），不复制、不改变 `PATH` / 包管理器设置；
//! - `runtime.json` 记录 `desktopVersion` / `platform` / `arch` / 可选 `payloadDigest` /
//!   `python` / 可选 `node`·`pnpm` / `pythonPackages` 发行版本映射；
//! - 平台或架构与运行进程不一致 → reject（不匹配即不可用）；
//! - 只读（仅查询路径与版本，无副作用）、非 deferred。
//!
//! 说明：另有「安装到 harness home」模式（`installPrimaryRuntime`，首次调用复制负载）。
//! 本实现采用**就地使用**模式（只读载体，如容器镜像层），不引入复制
//! 写入副作用；若未来需要 Desktop 式安装，可在 seam 上扩展副本路径。

pub mod manifest;
pub mod seam;
pub mod tools;

pub use seam::{WorkspaceDependencies, WorkspaceDepsService};
pub use tools::LoadWorkspaceDependenciesTool;

/// workspace 依赖查询错误。
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceDepsError {
    #[error("primary runtime: {0}")]
    InvalidMetadata(String),
    #[error("primary runtime: incompatible platform or architecture")]
    Incompatible,
    #[error("primary runtime: missing entry at {0}")]
    MissingEntry(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
