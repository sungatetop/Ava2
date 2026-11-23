//! 运行期模型目标（{ current, assembled } 双槽模型）。
//!
//! - `current`：UI / 命令写入的「下一请求目标」（`/model` 切换置换）；
//! - `assembled`：当前 step 进入 prompt-assembly 边界时对 `current` 的快照；
//! - 运行中切换立即生效于下一个请求，不中断运行中的 step。
//!
//! 使用 `ArcSwapOption` 承载 `current` / `assembled`：读侧无锁快照，写侧
//! `store` 原子置换，`OpenAiSseLlm` 与 `Kernel` 共享同一实例，跨线程安全。

use std::sync::Arc;

use arc_swap::ArcSwapOption;

/// 模型目标（provider 路由 + provider-owned model id + 可选 reasoning effort）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelTarget {
    /// provider-owned 模型 id（如 `Qwen3.8-27B`）。
    pub model: String,
    /// OpenAI 兼容基址（`/chat/completions` 前缀）。
    pub base_url: String,
    /// 鉴权密钥。
    pub api_key: String,
    /// 可选 reasoning effort（wire 层透传 `reasoning_effort`；None = 不发）。
    pub effort: Option<String>,
}

/// 可变模型目标引用。
pub struct ModelTargetRef {
    current: ArcSwapOption<ModelTarget>,
    assembled: ArcSwapOption<ModelTarget>,
}

impl ModelTargetRef {
    /// 以初始目标构造：`current` 与 `assembled` 同源（首个请求即用该目标）。
    pub fn new(initial: ModelTarget) -> Self {
        let current = Arc::new(initial);
        let assembled = current.clone();
        Self {
            current: ArcSwapOption::new(Some(current)),
            assembled: ArcSwapOption::new(Some(assembled)),
        }
    }

    /// 读取 `current`（下一请求目标）。
    pub fn current(&self) -> Option<ModelTarget> {
        self.current.load_full().map(|a| (*a).clone())
    }

    /// 读取 `assembled`（当前 step 快照目标）。
    pub fn assembled(&self) -> Option<ModelTarget> {
        self.assembled.load_full().map(|a| (*a).clone())
    }

    /// `/model` 切换：置换 `current`（下一请求目标），不中断运行中 step。
    pub fn swap_current(&self, target: ModelTarget) {
        self.current.store(Some(Arc::new(target)));
    }

    /// prompt-assembly 边界快照：`current` → `assembled`。
    #[allow(clippy::let_and_return)]
    pub fn snapshot(&self) {
        if let Some(current) = self.current.load_full() {
            self.assembled.store(Some(current));
        }
    }
}

#[cfg(test)]
#[path = "model_target_tests.rs"]
mod tests;
