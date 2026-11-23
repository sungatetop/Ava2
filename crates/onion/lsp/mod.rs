//! LSP 能力 seam：语言服务器 provider 注册 + 按扩展名
//! 选择 + 归一化查询，以及 `lsp` 工具。
//!
//! seam 是可注册 provider 的服务；Rust 单进程无真实语言服务器
//! 后端，因此实现为**可注册 provider 的空 seam**（缺省无 provider）。查询时按文件扩展名
//! 路由；无匹配 provider 时返回结构化 `LSP_UNAVAILABLE`。
//!
//! - `seam.rs`：seam 类型（operation/position/range/request/result）+ [`LspService`]；
//! - `render.rs`：纯渲染（location 分组 / hover / URI 解析 / 截断），无 I/O；
//! - `tools.rs`：`lsp` 工具——四操作、one-based→zero-based 坐标转换、渲染、超时。
//!
//! 核心语义：
//! - 只暴露四种语义操作（goToDefinition / findReferences / goToImplementation / hover），
//!   无 JSON-RPC 逃生舱；
//! - `findReferences` 恒含声明（provider 内部保证）；
//! - 位置 zero-based UTF-16 匹配线协议，模型面工具持有 one-based 光标约定。

mod render;
mod seam;
pub mod tools;

pub use render::{format_hover, format_locations, render_uri};
pub use seam::{
    final_extension, LspError, LspErrorCode, LspOperation, LspPosition, LspProvider, LspQueryRequest,
    LspQueryResult, LspRange, LspService,
};
pub use tools::LspTool;
