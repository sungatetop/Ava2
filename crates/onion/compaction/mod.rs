//! 上下文压缩模块：压力阈值派生 + 启发式估算 + LLM 摘要 + 区间替换事务 + listener 挂接。
//!
//! 设计文档：`ava/docs/design/onion-compaction-design.md`。

pub mod config;
pub mod estimate;
pub mod listener;
pub mod prune;
pub mod region;
pub mod summarizer;
pub mod types;
