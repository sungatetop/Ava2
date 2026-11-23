//! agent-team 配置（maxMembers/maxTasks/maxMessageBytes/disposalTimeoutMs）；
//! 均为可选、启动时校验、超限报 `TEAM_INVALID_CONFIG`。

use super::TeamError;

/// agent-team 默认配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeamConfig {
    /// 一个团队最多可创建的成员数（含失败成员），默认 16。
    pub max_members: u64,
    /// 任务板上最多活跃任务数，默认 256。
    pub max_tasks: u64,
    /// 一条消息的完整 sender-framed UTF-8 字节上限，默认 65536。
    pub max_message_bytes: usize,
    /// 关闭清理允许的时间（毫秒），默认 5000。
    pub disposal_timeout_ms: u64,
}

impl Default for TeamConfig {
    fn default() -> Self {
        Self {
            max_members: 16,
            max_tasks: 256,
            max_message_bytes: 65_536,
            disposal_timeout_ms: 5_000,
        }
    }
}

impl TeamConfig {
    /// 校验各项均为正整数，非法 → `TEAM_INVALID_CONFIG`。
    pub fn validate(&self) -> Result<(), TeamError> {
        for v in [self.max_members, self.max_tasks, self.disposal_timeout_ms] {
            if v == 0 {
                return Err(TeamError::InvalidConfig);
            }
        }
        if self.max_message_bytes == 0 {
            return Err(TeamError::InvalidConfig);
        }
        Ok(())
    }
}
