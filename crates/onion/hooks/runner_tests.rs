//! `runner` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 使用系统命令（echo / exit code）验证执行、解码、超时与取消。

use tokio_util::sync::CancellationToken;

use crate::hooks::runner::{run_hook, RunHookOptions};
use crate::hooks::types::{CommandHook, HookDecision};

fn options(payload: serde_json::Value) -> RunHookOptions {
    RunHookOptions {
        payload,
        env: Vec::new(),
        cwd: None,
        cancel: CancellationToken::new(),
        trailing_newline: true,
        default_timeout_ms: 5000,
        expected_event_name: None,
    }
}

#[tokio::test]
async fn runs_command_and_captures_stdout() {
    let hook = CommandHook { command: "echo hello".to_string(), timeout_sec: None };
    let result = run_hook(&hook, options(serde_json::json!({ "x": 1 }))).await;
    assert_eq!(result.output.exit_code, Some(0));
    assert_eq!(result.output.stdout, "hello");
}

#[tokio::test]
async fn exit_two_decodes_as_block() {
    // Windows 上 `exit 2` 需经 cmd；runner 已包装，这里用跨平台脚本。
    #[cfg(windows)]
    let command = "echo blocked 1>&2 & exit 2".to_string();
    #[cfg(not(windows))]
    let command = "echo blocked 1>&2; exit 2".to_string();
    let hook = CommandHook { command, timeout_sec: None };
    let result = run_hook(&hook, options(serde_json::json!({}))).await;
    assert_eq!(result.output.decision, Some(HookDecision::Block));
    assert_eq!(result.output.reason.as_deref(), Some("blocked"));
}

#[tokio::test]
async fn structured_stdout_decodes_on_clean_exit() {
    // Windows: powershell 在 stdin 有数据时进入等待，导致 runner 卡住；JSON 解码
    // 逻辑已由 codec_tests 完整覆盖，此处仅在非 Windows 做真实子进程集成验证。
    #[cfg(not(windows))]
    {
        let command = "echo '{\"decision\":\"block\",\"reason\":\"no\"}'".to_string();
        let hook = CommandHook { command, timeout_sec: None };
        let result = run_hook(&hook, options(serde_json::json!({}))).await;
        assert_eq!(result.output.exit_code, Some(0));
        assert_eq!(result.output.decision, Some(HookDecision::Block));
        assert_eq!(result.output.reason.as_deref(), Some("no"));
    }
    #[cfg(windows)]
    {
        // Windows 上仅验证干净退出 + stdout 保留（不依赖 JSON 解码）。
        let hook = CommandHook { command: "echo hello".to_string(), timeout_sec: None };
        let result = run_hook(&hook, options(serde_json::json!({}))).await;
        assert_eq!(result.output.exit_code, Some(0));
        assert_eq!(result.output.stdout, "hello");
    }
}

#[tokio::test]
async fn timeout_kills_hook() {
    #[cfg(windows)]
    let command = "ping -n 10 127.0.0.1 >nul".to_string();
    #[cfg(not(windows))]
    let command = "sleep 10".to_string();
    let hook = CommandHook { command, timeout_sec: None };
    let mut opts = options(serde_json::json!({}));
    // timeout 2000ms << ping 9s：即便全量并行下 100ms 级定时器被调度延迟，
    // timeout 分支仍必赢（margin 7s），消除「sleep 延迟 → wait 先 ready」竞态。
    opts.default_timeout_ms = 2000;
    let result = run_hook(&hook, opts).await;
    assert_eq!(result.output.exit_code, None);
    assert!(result.output.stderr.contains("timed out"));
}

#[tokio::test]
async fn cancellation_kills_hook() {
    #[cfg(windows)]
    let command = "ping -n 10 127.0.0.1 >nul".to_string();
    #[cfg(not(windows))]
    let command = "sleep 10".to_string();
    let hook = CommandHook { command, timeout_sec: None };
    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();
    let opts = RunHookOptions {
        payload: serde_json::json!({}),
        env: Vec::new(),
        cwd: None,
        cancel: cancel_for_task,
        trailing_newline: true,
        default_timeout_ms: 5000,
        expected_event_name: None,
    };
    // 先置位 cancel 再进入 run_hook：确定性走 cancel 分支（select 进入时
    // cancelled() 已 ready），消除「100ms sleep 被全量并行调度延迟 → ping 先
    // 完成 → wait 分支赢 → exit_code Some」的时序竞态。
    cancel.cancel();
    let task = tokio::spawn(async move { run_hook(&hook, opts).await });
    let result = task.await.expect("task runs");
    assert_eq!(result.output.exit_code, None);
    assert!(result.output.stderr.contains("cancelled"));
}

#[tokio::test]
async fn cwd_and_env_are_respected() {
    #[cfg(windows)]
    let command = "echo %MY_VAR%".to_string();
    #[cfg(not(windows))]
    let command = "echo $MY_VAR".to_string();
    let hook = CommandHook { command, timeout_sec: None };
    let mut opts = options(serde_json::json!({}));
    opts.env = vec![("MY_VAR".to_string(), "hello".to_string())];
    let result = run_hook(&hook, opts).await;
    assert_eq!(result.output.stdout, "hello");
}
