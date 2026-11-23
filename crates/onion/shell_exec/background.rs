//! 后台执行器：把一次性命令晋升为后台任务。
//!
//! 与 [`OneshotExecutor`] 互补：oneshot 同步返回，本执行器 spawn 后台进程、
//! 注册进 `JobRegistry`、流式累积输出到 `OutputRing`，结束后 settle 并经
//! inbox 注入完成通知（`user/message`）。
//! `job_kill` 经本执行器的 `kill` 取消进程（registry 标 Killed + 取消 token）。
//!
//! 进程句柄与取消经每任务 `CancellationToken` 关联：`kill` 先以 first-wins 在
//! 注册表落地 Killed，再取消 token 让在飞任务 kill 子进程——已落地事实不被
//! 后续真实退出覆盖（settle first-wins）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use onion_jobs::{JobId, JobKind, JobOutcome, JobRegistry, JobSpec, KillResult, OutputRing, SessionId};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::inbox::Inbox;
use crate::types::AgentMessage;

use super::{ShellDialect, SpawnPlan};

/// 任务 id 分配器（进程级单调递增，避免同毫秒碰撞）。
static JOB_SEQ: AtomicU64 = AtomicU64::new(0);

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 后台任务执行器（进程级单例或会话级共享）。
pub struct BackgroundExecutor {
    registry: Arc<JobRegistry>,
    /// 在飞任务 → 取消句柄（`job_kill` 用它终止进程）。
    cancels: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// 完成通知注入点（宿主装配完成后经 `wire_inbox` 接线）。
    inbox: Arc<OnceLock<Arc<Inbox>>>,
}

impl BackgroundExecutor {
    pub fn new(registry: Arc<JobRegistry>) -> Self {
        Self {
            registry,
            cancels: Arc::new(Mutex::new(HashMap::new())),
            inbox: Arc::new(OnceLock::new()),
        }
    }

    /// 延迟接线 inbox：宿主装配完成后调用（进程完成通知注入点）。
    /// 未接线时完成通知静默跳过（进程照常落地与 settle）。
    pub fn wire_inbox(&self, inbox: Arc<Inbox>) {
        let _ = self.inbox.set(inbox);
    }

    /// spawn 一个后台进程并注册为 Running 任务。返回任务 id。
    /// spawn 计划解析或进程启动失败时注册表落地 Failed 并返回 Err。
    pub fn spawn(
        &self,
        dialect: ShellDialect,
        command: &str,
        cwd: Option<PathBuf>,
        owner: SessionId,
    ) -> Result<JobId, String> {
        // 解析 spawn 计划（program 解析失败不注册占位任务，直接报错）。
        let plan = dialect.spawn_plan(command)?;
        let SpawnPlan { program, args } = plan;
        let id = JobId(format!(
            "job-{}-{}",
            crate::session::epoch_millis(),
            JOB_SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        // 预注册（Running）；spawn 失败则落地 Failed。
        self.registry.register(
            JobSpec { id: id.clone(), owner, kind: JobKind::Shell, command: command.to_string() },
            OutputRing::new(),
        );

        let mut cmd = Command::new(&program);
        cmd.args(&args);
        if let Some(cwd) = &cwd {
            cmd.current_dir(cwd);
        }
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(error) => {
                let message = format!(
                    "failed to start {program}: {error} (is it installed and on PATH?)"
                );
                self.registry.settle(&id.0, JobOutcome::Failed { error: message.clone() });
                self.notify(&id, command, &format!("failed: {message}"));
                return Err(message);
            }
        };

        let cancel = CancellationToken::new();
        lock(&self.cancels).insert(id.0.clone(), cancel.clone());

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let registry = self.registry.clone();
        let cancels = self.cancels.clone();
        let inbox = self.inbox.clone();
        let id2 = id.clone();
        let command2 = command.to_string();
        tokio::spawn(async move {
            // 流式排空 stdout / stderr 到输出环（并发，避免子进程写满管道阻塞）。
            let drain_stdout = drain(registry.clone(), &id2.0, stdout, "");
            let drain_stderr = drain(registry.clone(), &id2.0, stderr, "[stderr] ");
            tokio::pin!(drain_stdout);
            tokio::pin!(drain_stderr);

            let outcome = tokio::select! {
                _ = cancel.cancelled() => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    JobOutcome::Killed
                }
                status = child.wait() => {
                    let code = status.ok().and_then(|s| s.code()).unwrap_or(-1);
                    JobOutcome::Exited { code }
                }
            };
            // 等输出排空后再 settle（保证 job_output 读到完整尾窗）。
            let (_, _) = tokio::join!(drain_stdout, drain_stderr);
            registry.settle(&id2.0, outcome.clone());
            // 清理取消句柄（任务已终态，kill 无需再取消）。
            lock(&cancels).remove(&id2.0);
            let summary = match &outcome {
                JobOutcome::Exited { code } => format!("exited with code {code}"),
                JobOutcome::Killed => "killed".to_string(),
                JobOutcome::Failed { error } => format!("failed: {error}"),
            };
            Self::notify_with(&inbox, &id2, &command2, &summary);
        });

        Ok(id)
    }

    /// 取消任务：注册表 first-wins 标 Killed + 取消 token 终止进程。
    /// 已结束 / 不存在的任务不覆盖（返回对应 KillResult）。
    pub fn kill(&self, id: &str) -> KillResult {
        let result = self.registry.kill(id);
        if let Some(cancel) = lock(&self.cancels).get(id).cloned() {
            cancel.cancel();
        }
        result
    }

    /// 完成通知注入 inbox（next-step + 唤醒，通知以 user/message 形态进 inbox）。
    fn notify(&self, id: &JobId, command: &str, summary: &str) {
        Self::notify_with(&self.inbox, id, command, summary);
    }

    fn notify_with(inbox: &Arc<OnceLock<Arc<Inbox>>>, id: &JobId, command: &str, summary: &str) {
        let Some(inbox) = inbox.get() else { return };
        let text = format!(
            "Background job {id} ({command}) {summary}. Collect output with job_output, stop with job_kill."
        );
        let _ = inbox.steer(AgentMessage::user(text));
    }
}

/// 逐行排空一个输出流到输出环（`read_line` 增量，长跑任务可渐进可见）。
async fn drain<R: AsyncRead + Unpin>(
    registry: Arc<JobRegistry>,
    id: &str,
    reader: Option<R>,
    prefix: &str,
) {
    let Some(reader) = reader else { return };
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => registry.push_output(id, &format!("{prefix}{line}")),
            Err(_) => break,
        }
    }
}
