//! 会话工作目录状态：读 / 改（`cd`）会话的当前工作目录。
//!
//! 核心语义：
//! - 省略 `cd` → 读当前工作目录；
//! - 提供 `cd` → 改当前工作目录，相对路径用当前目录解析；
//! - **权限根不变**：`cd` 目标必须解析到沙箱 root 内（「权限根不变」约束），
//!   逃逸 / 绝对路径越界一律拒绝；
//! - 现有已启动的 shell / 进程保持各自目录（"Existing shells and running
//!   processes keep their own directories"——由 shell 一次性执行器天然满足）；
//! - 每次变更 append 一条 `WorkingDir` durable 事件（session-owned），重放恢复；
//! - 无 owning session（非 agent 调用方）拒绝。
//!
//! working directory 是**会话级状态**（非文件系统写入）：`cd` 只改会话记录的
//! cwd，不触碰文件系统；沙箱 root（权限根）恒定不变，只允许在其内部移动。

pub mod tools;

pub use tools::WorkingDirectoryTool;
