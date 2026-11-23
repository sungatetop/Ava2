//! present 领域模块：交付物声明工具 + durable 投影。
//!
//! - `tools.rs`：`present` 工具（声明已有文件为最终交付物，session-owned）；
//! - 成功后 append 一条 `deliverables/presented` durable 事件
//!   （`SessionEvent::Presented`）。
//!
//! 核心语义（教学子集）：
//! - 输入 `files: [{path, description}]`（1 至 `max_files` 个，推荐 1-4）；
//! - 相对路径用沙箱 working directory 解析（onion 沙箱 root = 会话 cwd）；
//! - 校验每个文件为**已存在的 regular file**（非目录/符号链接），否则拒绝；
//! - 只读：不复制文件内容（用户直接打开当前文件）；
//! - 成功后 append durable 事件 + 返回 `{turn, files}`。
//!
//! 教学简化：不实现 open-turn 边界检查（onion 单 agent 简化，无 turn 边界
//! 投影）——turn 取调用时已记录回合数。

pub mod tools;

pub use tools::PresentTool;
