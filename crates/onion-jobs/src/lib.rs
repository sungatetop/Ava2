//! onion-jobs：kind 无关的后台任务注册表。
//!
//! 只管状态与归属，不持有执行器（进程/PTY 归宿主 `shell_exec`）：故可脱离
//! onion 内核独立编译与测试。控制面工具（`job_list` / `job_output` / `job_kill`）
//! 是本 crate 类型之上的薄壳。

mod output;
mod registry;
mod types;

pub use output::OutputRing;
pub use registry::JobRegistry;
pub use types::{JobId, JobKind, JobOutcome, JobSpec, JobStatus, JobView, KillResult, SessionId};

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
