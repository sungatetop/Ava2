//! `session` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

fn approve_kind() -> PendingKind {
    PendingKind::Approval {
        tool: "shell".to_string(),
        input: serde_json::json!({ "cmd": "ls" }),
        reason: None,
    }
}

fn plan_kind() -> PendingKind {
    PendingKind::PlanReview { plan: "# plan".to_string() }
}

fn approval_sender() -> ReplySender {
    let (tx, _rx) = oneshot::channel();
    ReplySender::Approval(tx)
}

fn plan_sender() -> ReplySender {
    let (tx, _rx) = oneshot::channel();
    ReplySender::PlanReview(tx)
}

#[test]
fn resolve_unknown_request_is_not_found() {
    let channels = SessionChannels::new(true);
    assert!(matches!(
        channels.resolve("req-unknown", "approve"),
        Err(ResolveError::NotFound)
    ));
    assert_eq!(None, channels.resolved_decision("req-unknown"));
}

#[test]
fn resolve_is_idempotent_and_records_first_decision() {
    let channels = SessionChannels::new(true);
    let id = channels.push(approve_kind(), approval_sender());

    let views = channels.pending_views();
    assert_eq!(1, views.len());
    assert_eq!(id, views[0].0);
    assert_eq!("approval", views[0].1.label());

    assert!(channels.resolve(&id, "approve").is_ok());
    assert!(channels.pending_views().is_empty());
    assert_eq!(Some("approve".to_string()), channels.resolved_decision(&id));

    assert!(matches!(
        channels.resolve(&id, "deny"),
        Err(ResolveError::AlreadyResolved)
    ));
    // 重复回填不覆盖首次决定。
    assert_eq!(Some("approve".to_string()), channels.resolved_decision(&id));
}

#[test]
fn pending_views_list_every_registered_request() {
    let channels = SessionChannels::new(true);
    let first = channels.push(plan_kind(), plan_sender());
    let second = channels.push(approve_kind(), approval_sender());

    let views = channels.pending_views();
    let ids: Vec<String> = views.iter().map(|(id, ..)| id.clone()).collect();
    assert_eq!(2, ids.len());
    assert!(ids.contains(&first));
    assert!(ids.contains(&second));
    let plan_label = views
        .iter()
        .find(|(id, ..)| id == &first)
        .map(|(_, kind, _)| kind.label());
    assert_eq!(Some("plan_review"), plan_label);
}

/// 会话中断撤回全部待回填：表清空、答复端被 drop（等待端 recv Err → `cancelled`），
/// 请求 id 一并进入幂等窗口。
#[tokio::test]
async fn cancel_all_aborts_pending_and_records_cancelled() {
    let channels = SessionChannels::new(true);
    let (tx, rx) = oneshot::channel();
    let id = channels.push(approve_kind(), ReplySender::Approval(tx));
    assert_eq!(1, channels.pending_views().len());

    assert_eq!(1, channels.cancel_all("cancelled"));
    assert!(channels.pending_views().is_empty(), "待回填表应被清空");
    // 答复端被 drop：内核等待端按 recv Err 走 `cancelled` 结局。
    assert!(rx.await.is_err(), "答复端应已 drop");
    assert_eq!(Some("cancelled".to_string()), channels.resolved_decision(&id));
    // 中断后迟到的回填拿到 409 语义而非 404。
    assert!(matches!(
        channels.resolve(&id, "approve"),
        Err(ResolveError::AlreadyResolved)
    ));
    // 空表调用幂等。
    assert_eq!(0, channels.cancel_all("cancelled"));
}

/// 回填晚到（答复端已 drop）时把幂等记录纠正为 `cancelled`。
#[test]
fn override_decision_rewrites_resolved_record() {
    let channels = SessionChannels::new(true);
    let id = channels.push(approve_kind(), approval_sender());
    assert!(channels.resolve(&id, "approve").is_ok());
    assert_eq!(Some("approve".to_string()), channels.resolved_decision(&id));

    channels.override_decision(&id, "cancelled");
    assert_eq!(Some("cancelled".to_string()), channels.resolved_decision(&id));
    // 未记录的 id 静默忽略。
    channels.override_decision("req-unknown", "cancelled");
    assert_eq!(None, channels.resolved_decision("req-unknown"));
}

#[tokio::test]
async fn notices_fan_out_to_subscribers() {
    let channels = SessionChannels::new(false);
    let mut rx = channels.subscribe();
    channels.notice(SessionNotice::SessionClosed {
        reason: "done".to_string(),
    });
    let notice = rx.recv().await.expect("notice");
    assert_eq!("session_closed", notice.event_name());
}

#[test]
fn last_report_roundtrips() {
    let channels = SessionChannels::new(true);
    assert_eq!(None, channels.last_report());
    let summary = TurnSummary {
        turn: 2,
        steps: 3,
        reason: "completed".to_string(),
        usage: onion::types::Usage { input_tokens: 120, output_tokens: 30, ..Default::default() },
    };
    channels.set_last_report(summary.clone());
    assert_eq!(Some(summary), channels.last_report());
}

#[test]
fn stored_session_ms_requires_current_log_file() {
    let root = std::env::temp_dir().join(format!("onion_server_stored_{}", std::process::id()));
    let session_dir = root.join("s1");
    std::fs::create_dir_all(&session_dir).expect("临时会话目录");
    // 只有目录、没有当前版本日志：视为磁盘无此会话（与 list_sessions 同判定）。
    assert_eq!(None, stored_session_ms(&root, "s1"));
    std::fs::write(onion::paths::session_log_file(&session_dir), "").expect("写日志文件");
    assert!(stored_session_ms(&root, "s1").is_some());
    let _ = std::fs::remove_dir_all(&root);
}

/// 构造只用于 `resolve_root` 的注册表（不装配任何会话）。
fn registry_with_root(workspace_root: PathBuf) -> SessionRegistry {
    let sessions_dir = onion::paths::sessions_dir(&onion::paths::data_home(), &workspace_root);
    SessionRegistry::new(
        Config::from_env(),
        workspace_root,
        sessions_dir,
        Arc::new(crate::bridge::Bridge::new(
            Vec::new(),
            "http://127.0.0.1:0".to_string(),
            None,
        )),
    )
}

#[test]
fn resolve_root_defaults_to_workspace_root() {
    let base = std::env::temp_dir().join(format!("onion_ws_default_{}", std::process::id()));
    std::fs::create_dir_all(&base).expect("临时工作目录");
    let registry = registry_with_root(base.clone());
    assert_eq!(base, registry.resolve_root(None).expect("缺省根"));
    assert_eq!(base, registry.resolve_root(Some("   ")).expect("空白视为缺省"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn resolve_root_accepts_existing_absolute_directory() {
    let base = std::env::temp_dir().join(format!("onion_ws_abs_{}", std::process::id()));
    let child = base.join("ws-a");
    std::fs::create_dir_all(&child).expect("临时工作目录");
    let registry = registry_with_root(base.clone());
    let resolved = registry
        .resolve_root(Some(&child.display().to_string()))
        .expect("已存在的绝对目录");
    // 保留调用方拼写（不做 canonicalize，避免 Windows `\\?\` 前缀污染容器名）。
    assert_eq!(child, resolved);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn resolve_root_rejects_relative_and_missing_paths() {
    let base = std::env::temp_dir().join(format!("onion_ws_bad_{}", std::process::id()));
    std::fs::create_dir_all(&base).expect("临时工作目录");
    let registry = registry_with_root(base.clone());
    for raw in ["relative/ws", "ws-a"] {
        let error = registry.resolve_root(Some(raw)).expect_err("相对路径应拒绝");
        assert_eq!(std::io::ErrorKind::InvalidInput, error.kind());
    }
    let missing = base.join("not-created");
    let error = registry
        .resolve_root(Some(&missing.display().to_string()))
        .expect_err("不存在的目录应拒绝（不做隐式建目录）");
    assert_eq!(std::io::ErrorKind::InvalidInput, error.kind());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn locate_prefers_default_container_and_misses_unknown() {
    let base = std::env::temp_dir().join(format!("onion_ws_locate_{}", std::process::id()));
    let default_dir = base.join("default");
    std::fs::create_dir_all(&default_dir).expect("默认容器");
    let registry = SessionRegistry::new(
        Config::from_env(),
        base.clone(),
        default_dir.clone(),
        Arc::new(crate::bridge::Bridge::new(
            Vec::new(),
            "http://127.0.0.1:0".to_string(),
            None,
        )),
    );
    // 兜底容器优先：默认容器里的会话直接命中。
    let default_session = onion::paths::session_dir(&default_dir, "s-default");
    std::fs::create_dir_all(&default_session).expect("会话目录");
    std::fs::write(onion::paths::session_log_file(&default_session), "").expect("写日志");
    let (dir, _) = registry.locate("s-default").expect("默认容器命中");
    assert_eq!(default_dir, dir);
    // 磁盘上不存在的会话：兜底容器与全量扫描都查不到。
    assert!(registry.locate("s-unknown").is_none());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn container_root_restores_root_from_marker() {
    let base = std::env::temp_dir().join(format!("onion_container_root_{}", std::process::id()));
    let workspace = base.join("ws-alpha");
    let container = base.join("sessions").join("c--tmp-ws-alpha");
    std::fs::create_dir_all(&workspace).expect("临时工作目录");
    write_workspace_marker(&container, &workspace);
    // 自定义容器：以创建期写入的 cwd 标记为准（容器名有损，不可反解）。
    assert_eq!(workspace, container_root(&container, &base.join("default"), &base));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn container_root_falls_back_when_marker_missing_or_stale() {
    let base = std::env::temp_dir().join(format!("onion_container_fallback_{}", std::process::id()));
    let default_dir = base.join("sessions").join("default");
    let legacy = base.join("sessions").join("c--tmp-legacy");
    let stale = base.join("sessions").join("c--tmp-stale");
    std::fs::create_dir_all(&default_dir).expect("默认容器");
    std::fs::create_dir_all(&legacy).expect("旧容器");
    // 标记指向已删除目录：视为失效，回落兜底根（避免把权限根指向不存在的位置）。
    write_workspace_marker(&stale, &base.join("gone"));
    let fallback = base.clone();
    assert_eq!(fallback, container_root(&legacy, &default_dir, &fallback));
    assert_eq!(fallback, container_root(&stale, &default_dir, &fallback));
    // 兜底容器恒用兜底根，不读标记。
    assert_eq!(fallback, container_root(&default_dir, &default_dir, &fallback));
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn open_prefers_live_session_over_disk_locate() {
    let base = std::env::temp_dir().join(format!("onion_ws_live_{}", std::process::id()));
    let default_dir = base.join("default");
    std::fs::create_dir_all(&default_dir).expect("临时容器目录");
    let registry = SessionRegistry::new(
        Config::from_env(),
        base.clone(),
        default_dir.clone(),
        Arc::new(crate::bridge::Bridge::new(
            Vec::new(),
            "http://127.0.0.1:0".to_string(),
            None,
        )),
    );
    let session = registry
        .create(Some("s-live".to_string()), None, false, None)
        .await
        .expect("装配会话");
    // 删除落盘日志：live 会话不应因此被判不存在（磁盘 locate 失败 ≠ 会话消失，
    // 对应「并发测试误删共享 ONION_HOME」引发的 session_not_found 误报）。
    let session_dir = onion::paths::session_dir(&session.sessions_dir, "s-live");
    assert!(session_dir.exists(), "装配后应已落盘");
    std::fs::remove_dir_all(&session_dir).expect("删除落盘目录");
    let reopened = registry
        .open("s-live", false)
        .await
        .expect("open 成功")
        .expect("live 会话仍可打开");
    assert!(Arc::ptr_eq(&session, &reopened), "应命中同一 live 实例而非冷恢复");
    // 内存与磁盘皆无：仍返回 None。
    assert!(registry
        .open("s-unknown", false)
        .await
        .expect("open 成功")
        .is_none());
    let _ = std::fs::remove_dir_all(&base);
}
