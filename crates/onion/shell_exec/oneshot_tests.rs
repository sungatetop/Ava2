//! `oneshot` 模块单元测试：一次性进程执行（exit code / stderr 摘要 / 超时归一 / spill）。

use std::path::PathBuf;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::oneshot::OneshotExecutor;
use super::ShellDialect;

/// 探测可用的 shell 方言（复用运行时 `resolve_program`：Windows 排除 WSL 启动器、
/// pwsh 回落 powershell）。探不到返回 None（跳过断言）。
fn probe_dialect() -> Option<ShellDialect> {
    [ShellDialect::Bash, ShellDialect::Pwsh]
        .into_iter()
        .find(|dialect| dialect.resolve_program().is_ok())
}

/// 方言适切的「输出某字符串」命令：bash 用 printf，pwsh 用 Write-Output。
fn echo_cmd(dialect: ShellDialect, text: &str) -> String {
    match dialect {
        ShellDialect::Bash => format!("printf '{text}'"),
        ShellDialect::Pwsh => format!("Write-Output {text}"),
    }
}

/// 方言适切的「写 stderr」命令：bash 用 `1>&2`，pwsh 用 Write-Error。
fn stderr_cmd(dialect: ShellDialect) -> String {
    match dialect {
        ShellDialect::Bash => "echo boom 1>&2".to_string(),
        ShellDialect::Pwsh => "Write-Error boom".to_string(),
    }
}

/// 以探测到的方言执行命令。
async fn run(
    executor: &OneshotExecutor,
    dialect: ShellDialect,
    command: &str,
) -> Result<super::ExecOutcome, String> {
    let cancel = CancellationToken::new();
    executor.exec(dialect, command, None, Some(Duration::from_secs(10)), cancel).await
}

/// bash 方言 argv：`-c <command>`（Windows 上需 Git Bash，无则跳过）。
#[test]
fn spawn_plan_bash_uses_dash_c() {
    let Ok(plan) = ShellDialect::Bash.spawn_plan("echo hi") else { return };
    assert_eq!(plan.args, vec!["-c".to_string(), "echo hi".to_string()]);
    // 不得解析到 WSL 启动器（System32 系统组件 / WindowsApps Store 别名）。
    let program = plan.program.to_lowercase();
    for launcher in ["system32\\bash", "windowsapps\\bash"] {
        assert!(!program.contains(launcher), "resolved to WSL launcher ({launcher}): {}", plan.program);
    }
}

/// PowerShell argv：`-EncodedCommand` + UTF-8 前缀 + exit 尾缀（对齐 ava-toolkit）。
#[test]
fn spawn_plan_pwsh_uses_encoded_command() {
    use base64::Engine as _;
    let Ok(plan) = ShellDialect::Pwsh.spawn_plan("Write-Output hi") else { return };
    assert_eq!(plan.args.len(), 4);
    assert_eq!(plan.args[0], "-NoProfile");
    assert_eq!(plan.args[1], "-NonInteractive");
    assert_eq!(plan.args[2], "-EncodedCommand");
    let bytes = base64::engine::general_purpose::STANDARD.decode(&plan.args[3]).expect("b64");
    let utf16: Vec<u16> =
        bytes.as_chunks::<2>().0.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
    let script = String::from_utf16(&utf16).expect("utf16");
    assert!(script.starts_with("$ProgressPreference='SilentlyContinue';"));
    assert!(script.contains("[Console]::OutputEncoding=[System.Text.Encoding]::UTF8"));
    assert!(script.ends_with("exit $LASTEXITCODE"));
    assert!(script.contains("Write-Output hi"));
}

#[tokio::test]
async fn oneshot_captures_exit_code_and_stdout() {
    let Some(dialect) = probe_dialect() else { return };
    let executor = OneshotExecutor::new(None);
    let outcome = run(&executor, dialect, &echo_cmd(dialect, "hello")).await.expect("exec failed");
    assert_eq!(outcome.exit_code, 0);

    let outcome = run(&executor, dialect, "exit 3").await.expect("exec failed");
    assert_eq!(outcome.exit_code, 3);
}

#[tokio::test]
async fn oneshot_times_out_and_normalizes_exit_code() {
    let Some(dialect) = probe_dialect() else { return };
    let executor = OneshotExecutor::new(None);
    let cancel = CancellationToken::new();
    // 短超时 + 长命令 → timed_out=true, exit_code=-1（`sleep` 对 bash/pwsh 通用）。
    let outcome = executor
        .exec(dialect, "sleep 30", None, Some(Duration::from_millis(50)), cancel)
        .await
        .expect("exec failed");
    assert!(outcome.timed_out, "expected timeout");
    assert_eq!(outcome.exit_code, -1);
}

#[tokio::test]
async fn oneshot_captures_stderr_in_result_text() {
    let Some(dialect) = probe_dialect() else { return };
    let executor = OneshotExecutor::new(None);
    let outcome = run(&executor, dialect, &stderr_cmd(dialect)).await.expect("exec failed");
    // stderr 摘要进入结果正文。
    assert!(outcome.stdout.contains("boom"), "stderr not in text: {}", outcome.stdout);
}

#[tokio::test]
async fn oneshot_spills_full_output_to_job_log() {
    let root = std::env::temp_dir().join(format!("ava_onion_spill_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let spill_dir = root.join("logs").join("jobs");
    let executor = OneshotExecutor::new(Some(spill_dir.clone()));
    let Some(dialect) = probe_dialect() else { return };
    let outcome = run(&executor, dialect, &echo_cmd(dialect, "data-xyz")).await.expect("exec failed");
    // 有输出 → 落盘并提示路径。
    assert!(outcome.stdout.contains("full output saved to"), "no spill hint: {}", outcome.stdout);
    assert!(outcome.truncated, "spill marks truncated");
    // 目录下应有日志文件。
    let files: Vec<PathBuf> = std::fs::read_dir(&spill_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert!(!files.is_empty(), "no spill file written");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn oneshot_cancel_kills_and_normalizes() {
    let Some(dialect) = probe_dialect() else { return };
    let executor = OneshotExecutor::new(None);
    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();
    let task = tokio::spawn(async move {
        executor.exec(dialect, "sleep 30", None, Some(Duration::from_secs(30)), cancel_for_task).await
    });
    // 稍后取消。
    tokio::time::sleep(Duration::from_millis(80)).await;
    cancel.cancel();
    let outcome = task.await.expect("task panicked").expect("exec failed");
    assert_eq!(outcome.exit_code, -1, "cancel should normalize exit code");
}
