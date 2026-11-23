//! 三层显式状态机：Session / Turn / Step。非法转移拒绝（确定性），合法转移发 StateChanged。

use std::fmt;

use crate::listener::EventSink;
use crate::session::{EventBus, LiveEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Preparing,
    Requesting,
    Streaming,
    Executing,
    Settling,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnState {
    Claiming,
    Admitting,
    Stepping(StepState),
    Stopping,
    Closing,
}

/// Session 层状态机（Running 覆盖一轮 turn 的全寿命）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Running(TurnState),
    Ended,
}

impl fmt::Display for StepState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Preparing => "Preparing",
            Self::Requesting => "Requesting",
            Self::Streaming => "Streaming",
            Self::Executing => "Executing",
            Self::Settling => "Settling",
        };
        f.write_str(label)
    }
}

impl fmt::Display for TurnState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Claiming => f.write_str("Claiming"),
            Self::Admitting => f.write_str("Admitting"),
            Self::Stepping(step) => write!(f, "Stepping({step})"),
            Self::Stopping => f.write_str("Stopping"),
            Self::Closing => f.write_str("Closing"),
        }
    }
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => f.write_str("Idle"),
            Self::Running(turn) => write!(f, "Running({turn})"),
            Self::Ended => f.write_str("Ended"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateError {
    pub from: String,
    pub via: &'static str,
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "非法状态转移：{} --{}--> ?", self.from, self.via)
    }
}

impl std::error::Error for StateError {}

/// 状态机：持有当前态，合法转移时广播 StateChanged（live，不进 durable）。
pub struct Machine {
    state: SessionState,
    bus: EventBus,
    sink: Option<EventSink>,
}

impl Machine {
    pub fn new(bus: EventBus) -> Self {
        Self { state: SessionState::Idle, bus, sink: None }
    }

    /// 注入 listener 扇出 sink（Runtime 装配期调用）。
    pub fn set_sink(&mut self, sink: EventSink) {
        self.sink = Some(sink);
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn snapshot(&self) -> String {
        self.state.to_string()
    }

    /// 校验并转移；非法转移拒绝（含 from == to 的伪转移），合法转移发 StateChanged。
    fn go(&mut self, next: SessionState, via: &'static str) -> Result<(), StateError> {
        let from = self.state;
        if !Self::legal(from, next) {
            return Err(StateError { from: from.to_string(), via });
        }
        self.state = next;
        let live = LiveEvent::StateChanged {
            from: from.to_string(),
            to: next.to_string(),
        };
        self.bus.send(live.clone());
        if let Some(sink) = &self.sink {
            sink(&live);
        }
        Ok(())
    }

    fn legal(from: SessionState, next: SessionState) -> bool {
        matches!(
            (from, next),
            // Session 层
            (SessionState::Idle, SessionState::Running(TurnState::Claiming))
                | (SessionState::Running(TurnState::Closing), SessionState::Idle)
                | (_, SessionState::Ended)
                // Turn 层
                | (SessionState::Running(TurnState::Claiming), SessionState::Running(TurnState::Admitting))
                | (SessionState::Running(TurnState::Claiming), SessionState::Running(TurnState::Closing))
                | (SessionState::Running(TurnState::Admitting), SessionState::Running(TurnState::Stepping(StepState::Preparing)))
                | (SessionState::Running(TurnState::Admitting), SessionState::Running(TurnState::Closing))
                | (SessionState::Running(TurnState::Stepping(StepState::Settling)), SessionState::Running(TurnState::Admitting))
                | (SessionState::Running(TurnState::Stepping(StepState::Settling)), SessionState::Running(TurnState::Stopping))
                | (SessionState::Running(TurnState::Stopping), SessionState::Running(TurnState::Closing))
                | (SessionState::Running(TurnState::Stopping), SessionState::Running(TurnState::Admitting))
                // Step 层
                | (SessionState::Running(TurnState::Stepping(StepState::Preparing)), SessionState::Running(TurnState::Stepping(StepState::Requesting)))
                | (SessionState::Running(TurnState::Stepping(StepState::Preparing)), SessionState::Running(TurnState::Stepping(StepState::Settling)))
                | (SessionState::Running(TurnState::Stepping(StepState::Requesting)), SessionState::Running(TurnState::Stepping(StepState::Streaming)))
                | (SessionState::Running(TurnState::Stepping(StepState::Streaming)), SessionState::Running(TurnState::Stepping(StepState::Requesting)))
                | (SessionState::Running(TurnState::Stepping(StepState::Streaming)), SessionState::Running(TurnState::Stepping(StepState::Executing)))
                | (SessionState::Running(TurnState::Stepping(StepState::Streaming)), SessionState::Running(TurnState::Stepping(StepState::Settling)))
                | (SessionState::Running(TurnState::Stepping(StepState::Executing)), SessionState::Running(TurnState::Stepping(StepState::Settling)))
                // 取消/错误/上限：Stepping 任意子态直接进入 Closing
                | (SessionState::Running(TurnState::Stepping(StepState::Preparing)), SessionState::Running(TurnState::Closing))
                | (SessionState::Running(TurnState::Stepping(StepState::Requesting)), SessionState::Running(TurnState::Closing))
                | (SessionState::Running(TurnState::Stepping(StepState::Streaming)), SessionState::Running(TurnState::Closing))
                | (SessionState::Running(TurnState::Stepping(StepState::Executing)), SessionState::Running(TurnState::Closing))
                | (SessionState::Running(TurnState::Stepping(StepState::Settling)), SessionState::Running(TurnState::Closing))
        )
    }

    pub fn start_run(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Claiming), "start_run")
    }

    pub fn end(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Ended, "end")
    }

    pub fn claimed(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Admitting), "claimed")
    }

    /// 无任何输入：不开轮（run 返回 None）。
    pub fn no_input(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Closing), "no_input")
    }

    pub fn enter_step(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Stepping(StepState::Preparing)), "enter_step")
    }

    /// pre-step reject 或空批次：领取保持移除，空轮关闭。
    pub fn reject_step(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Closing), "reject_step")
    }

    /// 取消/错误/上限中止：Claiming、Admitting、Stepping 任意子态都可进入 Closing。
    pub fn abort_step(&mut self) -> Result<(), StateError> {
        match self.state {
            SessionState::Running(TurnState::Claiming)
            | SessionState::Running(TurnState::Admitting)
            | SessionState::Running(TurnState::Stepping(_)) => {
                self.go(SessionState::Running(TurnState::Closing), "abort_step")
            }
            _ => Err(StateError { from: self.state.to_string(), via: "abort_step" }),
        }
    }

    pub fn enter_requesting(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Stepping(StepState::Requesting)), "enter_requesting")
    }

    pub fn retry_request(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Stepping(StepState::Requesting)), "retry_request")
    }

    pub fn enter_streaming(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Stepping(StepState::Streaming)), "enter_streaming")
    }

    pub fn enter_executing(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Stepping(StepState::Executing)), "enter_executing")
    }

    pub fn enter_settling(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Stepping(StepState::Settling)), "enter_settling")
    }

    /// step 收尾且欠请求（工具）或 next-step 有输入：回 Admitting 开下一 step。
    pub fn step_continues(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Admitting), "step_continues")
    }

    pub fn enter_stopping(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Stopping), "enter_stopping")
    }

    /// turn-stopping 窗口后无新输入：闭轮。
    pub fn stop_end(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Closing), "stop_end")
    }

    /// turn-stopping 窗口内注入了新输入：回 Admitting 续跑。
    pub fn stop_continue(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Running(TurnState::Admitting), "stop_continue")
    }

    /// 轮收尾完成：交还 Session 层（Idle）。
    pub fn turn_closed(&mut self) -> Result<(), StateError> {
        self.go(SessionState::Idle, "turn_closed")
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
