//! view_state.rs：视图级开关（设计文档 §7.3 terminal-only controls）。
//!
//! 全部是**视图级**状态：只影响投影渲染，不回写 durable 日志、不进模型上下文。
//!
//! - `Ctrl+R`：推理（thinking）显隐切换；
//! - `Ctrl+E`：工具卡片统一展开/折叠；
//! - `Ctrl+L`：强制重绘（组件树重建 + repaint，由主循环无条件 redraw 兑现）；
//! - `Ctrl+U`：transcript 视图级清空（durable 日志不动）。
//!
//! 推理默认隐藏、工具卡片默认折叠：长输出与思考链默认不占屏，需要时再展开。

/// 视图级开关集合（随会话重建，不跨会话保留）。
/// derive 默认 = 推理隐藏 + 工具折叠（bool::default 即 false，语义一致）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewState {
    /// 是否渲染 assistant 的推理链。
    pub show_thinking: bool,
    /// 工具卡片是否展开完整输出（折叠态只出标题行）。
    pub expand_tools: bool,
}

impl ViewState {
    /// `Ctrl+R`：推理显隐切换，返回切换后的可见性。
    pub fn toggle_thinking(&mut self) -> bool {
        self.show_thinking = !self.show_thinking;
        self.show_thinking
    }

    /// `Ctrl+E`：工具卡片统一展开/折叠，返回切换后的展开态。
    pub fn toggle_tools(&mut self) -> bool {
        self.expand_tools = !self.expand_tools;
        self.expand_tools
    }
}

#[cfg(test)]
#[path = "view_state_tests.rs"]
mod tests;
