//! workflow 编排。
//!
//! 与经典脚本式编排的差异（诚实降级）：脚本式的 `workflow` 工具接收一段 JavaScript
//! 脚本（`agent()` / `pipeline` / `parallel` / `phase` / `log` 钩子），由沙箱内
//! 的 JS 运行时执行；本实现不引入 JS 运行时，改为接收**声明式 JSON 程序**
//! （[`Program`]）：
//!
//! - `steps` **串行**执行，每步经 [`SubagentRuntime::run_round`] 派生子代理；
//! - 步骤 `for_each` 指向 `args` 内的数组，逐项**并发**派发（信号量限流），
//!   该步骤产出数组；其余步骤产出该子代理最终文本；
//! - `phase` 仅作进度分组标签（标题在 `meta.phases` 声明），不改变执行顺序；
//! - 模板占位符 `{{args.<路径>}}` / `{{steps.<id>[.<路径>]}}` / `{{item[.<路径>]}}`
//!   / `{{index}}` 在派发前替换，未解析即报错（不静默留空）；
//! - 对齐点：单个 agent 失败解析为 `null`（不终止整个 workflow）；终态三态
//!   `completed` / `cancelled` / `error`（非 completed 一律按 is_error 呈示）。
//!
//! [`SubagentRuntime::run_round`]: crate::subagent::runtime::SubagentRuntime::run_round

pub mod program;
pub mod runner;

pub use program::{
    DEFAULT_CONCURRENCY, MAX_CONCURRENCY, Phase, Program, ProgramMeta, Step, TemplateContext,
};
pub use runner::{run, result_view, Progress, RunReport, StopReason};

#[cfg(test)]
#[path = "program_tests.rs"]
mod program_tests;

#[cfg(test)]
#[path = "runner_tests.rs"]
mod runner_tests;
