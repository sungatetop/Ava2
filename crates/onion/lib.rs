//! onion 洋葱模型内核库。
//!
//! 原为 `ava-core/examples/onion` 的教学参考实现，现提升为独立 crate `onion`。
//! 包含洋葱模型的内核、状态机、上下文/瀑布、压缩、钩子、goal/plan 模式、
//! subagent 中继、沙箱与工具面等全部模块；交互入口见 `onion-cli`。

// 教学参照刻意暴露完整 API 面（宿主句柄、观测入口等）：demo 主路径未
// 用到的成员属参照实现的一部分，不视为死代码。
#![allow(dead_code)]

// 公共 API 返回类型（Runtime::cancel_handle 等）的跨 crate 可见性重导出。
pub use tokio_util::sync::CancellationToken;

pub mod approval;
pub mod attachment;
pub mod agent_team;
pub mod ask_user;
pub mod assemble;
pub mod compaction;
pub mod config;
pub mod context;
pub mod credentials;
pub mod fs_observation;
pub mod goal;
pub mod hooks;
pub mod hooks_claude;
pub mod inbox;
pub mod kernel;
pub mod listener;
pub mod listeners;
pub mod llm;
pub mod media;
pub mod memory;
pub mod model_target;
pub mod office;
pub mod paths;
pub mod plan;
pub mod present;
pub mod preset;
pub mod provider_catalog;
pub mod ralph;
pub mod reminders;
pub mod retry;
pub mod runtime;
pub mod sandbox;
pub mod schedule;
pub mod session;
pub mod session_cwd;
pub mod session_fork;
pub mod session_lock;
pub mod session_query;
pub mod settings;
pub mod shell_exec;
pub mod skill;
pub mod spill;
pub mod state;
pub mod subagent;
pub mod system_prompt;
pub mod todo;
pub mod tools;
pub mod types;
pub mod lsp;
pub mod mcp_resources;
pub mod terminal;
pub mod web;
pub mod waterfall;
pub mod workflow;
pub mod worktree;
pub mod working_dir;
pub mod workspace;
pub mod workspace_deps;
