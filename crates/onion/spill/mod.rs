//! 通用 spill 工件库（store / cleanup / policy 三件套）：
//!
//! - [`store::SpillStore`]：本地 FS 后端——超大工具结果等文本工件按
//!   `<root>/sessions/<会话 id>/<随机前缀>-<编码名>.txt` 互斥创建落盘，返回
//!   [`store::SpillRef`]（不透明 locator + 字节数 + 取回提示）；
//! - [`cleanup`]：进程启动 mtime 截止清扫 GC（缺省保留 30 天，只删普通文件，
//!   清空的会话目录连带修剪）；
//! - [`policy`]：溢出保留策略——估算 token 超阈值的工具结果在落 durable 前
//!   替换为「头尾保留 + 溢出提示」，原文落工件经 locator 读回。
//!
//! 工件按 mtime 而非会话生命周期回收：fork / 子会话对 locator 的引用在保留
//! 期内始终可读（fork 继承语义由此成立）。未接线 store 或 spill IO 失败
//! 时退化为原文直通——策略永不丢结果。

pub mod cleanup;
pub mod policy;
pub mod store;

/// spill 根下的会话目录名（store 落盘与 cleanup 清扫共用同一布局约定）。
pub(crate) const SESSIONS_DIR: &str = "sessions";

pub use store::{SpillRef, SpillStore};
