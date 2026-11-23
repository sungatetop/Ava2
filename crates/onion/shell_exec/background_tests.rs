//! `background` 模块单元测试：后台任务 spawn→输出环→settle→kill（经真实进程）。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use onion_jobs::{JobOutcome, JobRegistry, JobStatus, SessionId};
use tokio::time::sleep;

use super::background::BackgroundExecutor;
use super::ShellDialect;
use crate::inbox::Inbox;
use crate::session::EventBus;

/// 探测可用的 shell 方言（复用运行时 `resolve_program`：Windows 排除 WSL 启动器、
/// pwsh 回落 powershell）。探不到返回 None（跳过断言）。
fn probe_dialect() -> Option<ShellDialect> {
    [ShellDialect::Bash, ShellDialect::Pwsh]
        .into_iter()
        .find(|dialect| dialect.resolve_program().is_ok())
}

/// 方言适切命令：bash 用 printf / sleep，pwsh 用 Write-Output / Start-Sleep。
fn echo_cmd(dialect: ShellDialect, text: &str) -> String {
    match dialect {
        ShellDialect::Bash => format!("printf '{text}\\n'"),
        ShellDialect::Pwsh => format!("Write-Output {text}"),
    }
}

/// 方言适切「长跑」命令：bash 用 sleep，pwsh 用 Start-Sleep。
fn sleep_cmd(dialect: ShellDialect, secs: u64) -> String {
    match dialect {
        ShellDialect::Bash => format!("sleep {secs}"),
        ShellDialect::Pwsh => format!("Start-Sleep -Seconds {secs}"),
    }
}

/// 轮询等待任务落地终态（最长 `timeout`）。
async fn wait_settled(registry: &JobRegistry, id: &str, timeout: Duration) -> JobStatus {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(view) = registry.get(id) {
            if !view.status.is_running() {
                return view.status;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("job {id} did not settle within {timeout:?}");
        }
        sleep(Duration::from_millis(20)).await;
    }
}

/// 未接线 inbox 时完成通知静默跳过，进程照常落地。
#[tokio::test]
async fn background_spawn_settles_with_exit_code() {
    let Some(dialect) = probe_dialect() else { return };
    let registry = Arc::new(JobRegistry::new());
    let executor = BackgroundExecutor::new(registry.clone());
    let id = executor
        .spawn(dialect, &echo_cmd(dialect, "bg-hello"), None, SessionId("s1".to_string()))
        .expect("spawn failed");
    let status = wait_settled(&registry, &id.0, Duration::from_secs(10)).await;
    assert_eq!(status, JobStatus::Settled(JobOutcome::Exited { code: 0 }));
    // 输出进入输出环。
    let output = registry.read_output(&id.0, 100).unwrap_or_default();
    assert!(output.contains("bg-hello"), "unexpected output: {output}");
}

/// kill 在飞任务：first-wins 落地 Killed，且取消句柄终止进程。
#[tokio::test]
async fn background_kill_marks_killed() {
    let Some(dialect) = probe_dialect() else { return };
    let registry = Arc::new(JobRegistry::new());
    let executor = BackgroundExecutor::new(registry.clone());
    let id = executor
        .spawn(dialect, &sleep_cmd(dialect, 30), None, SessionId("s1".to_string()))
        .expect("spawn failed");
    // 稍等进程真正起来再 kill。
    sleep(Duration::from_millis(100)).await;
    let result = executor.kill(&id.0);
    assert_eq!(result, onion_jobs::KillResult::Requested);
    let status = wait_settled(&registry, &id.0, Duration::from_secs(10)).await;
    assert_eq!(status, JobStatus::Settled(JobOutcome::Killed));
    // 再次 kill：已落地，不覆盖。
    assert_eq!(executor.kill(&id.0), onion_jobs::KillResult::AlreadySettled);
    // 不存在的任务。
    assert_eq!(executor.kill("job-nonexistent"), onion_jobs::KillResult::NotFound);
}

/// owner 隔离：不同会话只看到自己的任务。
#[tokio::test]
async fn background_list_is_owner_scoped() {
    let Some(dialect) = probe_dialect() else { return };
    let registry = Arc::new(JobRegistry::new());
    let executor = BackgroundExecutor::new(registry.clone());
    let id_a = executor
        .spawn(dialect, &echo_cmd(dialect, "a"), None, SessionId("sA".to_string()))
        .expect("spawn a failed");
    let id_b = executor
        .spawn(dialect, &echo_cmd(dialect, "b"), None, SessionId("sB".to_string()))
        .expect("spawn b failed");
    let _ = wait_settled(&registry, &id_a.0, Duration::from_secs(10)).await;
    let _ = wait_settled(&registry, &id_b.0, Duration::from_secs(10)).await;
    let list_a = registry.list(&SessionId("sA".to_string()));
    assert_eq!(list_a.len(), 1);
    assert_eq!(list_a[0].spec.id, id_a);
    let list_b = registry.list(&SessionId("sB".to_string()));
    assert_eq!(list_b.len(), 1);
    assert_eq!(list_b[0].spec.id, id_b);
    // cleanup_owner 只清本会话。
    assert_eq!(registry.cleanup_owner(&SessionId("sA".to_string())), 1);
    assert_eq!(registry.list(&SessionId("sB".to_string())).len(), 1);
}

/// 接线 inbox 后，完成通知经 steer 注入 next-step 分区。
#[tokio::test]
async fn background_wired_inbox_notifies_on_completion() {
    let Some(dialect) = probe_dialect() else { return };
    let registry = Arc::new(JobRegistry::new());
    let executor = BackgroundExecutor::new(registry.clone());
    let inbox = Arc::new(Inbox::new(EventBus::new()));
    executor.wire_inbox(inbox.clone());
    let id = executor
        .spawn(dialect, &echo_cmd(dialect, "notify-me"), Some(PathBuf::from(".")), SessionId("s1".to_string()))
        .expect("spawn failed");
    let _ = wait_settled(&registry, &id.0, Duration::from_secs(10)).await;
    // 轮询等待通知进入 next-step 分区（异步 spawn 里注入）。
    for _ in 0..20 {
        let messages = inbox.drain_next_step();
        if !messages.is_empty() {
            let text = messages.iter().map(crate::types::AgentMessage::text_content).collect::<Vec<_>>().join("\n");
            assert!(text.contains(&id.0), "unexpected notify: {text}");
            return;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("completion notification not injected within timeout");
}
