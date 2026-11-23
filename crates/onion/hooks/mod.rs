//! 进程级 command hooks 协议层：matcher / runner / codec / merge / config。
//!
//! 共享、dialect 无关的钩子协议库。Claude Code
//! 桥（`hooks_claude.rs`）自持 payload、env、matcher 模式与 typed 决策映射。
//! 设计文档：`ava/docs/ava-core/plan/onion-command-hooks-implementation.md`。

pub mod codec;
pub mod config;
pub mod discover;
pub mod matcher;
pub mod merge;
pub mod runner;
pub mod types;
