//! `journal` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::{Arc, Mutex};

use crate::agent_team::journal::{AppendFn, Journal, Notify};
use crate::agent_team::projection::TeamEvent;
use crate::agent_team::types::{TeamContext, TeamMemberPhase, TeamMemberSnapshot};

fn member(id: &str, name: &str) -> TeamMemberSnapshot {
    TeamMemberSnapshot {
        id: id.to_string(),
        name: name.to_string(),
        description: "".into(),
        provider: "spawn".into(),
        context: TeamContext::Fresh,
        phase: TeamMemberPhase::Provisioning,
        error: None,
    }
}

#[tokio::test]
async fn transact_appends_and_applies() {
    let appended: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
    let appended2 = appended.clone();
    let append: AppendFn = Arc::new(move |e: TeamEvent| {
        appended2.lock().unwrap().push(e.type_name().to_string());
        Ok(())
    });
    let journal = Journal::new(append, Notify::new());
    let result: u32 = journal
        .transact(|_state| {
            Ok((
                vec![TeamEvent::Member(member("m1", "alice"))],
                42u32,
            ))
        })
        .await
        .unwrap();
    assert_eq!(result, 42);
    assert_eq!(*appended.lock().unwrap(), vec!["team/member".to_string()]);
    // 投影已应用。
    let state = journal.snapshot().await;
    assert_eq!(state.members.len(), 1);
    assert_eq!(state.members[0].name, "alice");
}

#[tokio::test]
async fn transact_serializes_sequence() {
    let journal = Journal::new(
        Arc::new(|_e: TeamEvent| Ok(())),
        Notify::new(),
    );
    // 连续两个事务，revision 连续。
    for (counter, _) in (0..3).enumerate() {
        let n = counter as u64 + 1;
        let _: u64 = journal
            .transact(move |state| {
                Ok((
                    vec![TeamEvent::Task(crate::agent_team::types::TeamTaskSnapshot {
                        id: format!("task-{}", state.next_task_number),
                        revision: 1,
                        subject: "t".into(),
                        description: "".into(),
                        status: crate::agent_team::types::TeamTaskStatus::Pending,
                        owner_id: None,
                        blocked_by: vec![],
                        write_scopes: vec![],
                    })],
                    n,
                ))
            })
            .await
            .unwrap();
    }
    let state = journal.snapshot().await;
    assert_eq!(state.tasks.len(), 3);
    assert_eq!(state.next_task_number, 4);
}

#[tokio::test]
async fn transact_propagates_append_error() {
    let journal = Journal::new(
        Arc::new(|_e: TeamEvent| Err("boom".to_string())),
        Notify::new(),
    );
    let result = journal
        .transact(|_state| Ok((vec![TeamEvent::Member(member("m1", "alice"))], ())))
        .await;
    assert!(result.is_err());
    // append 失败不应污染投影。
    let state = journal.snapshot().await;
    assert!(state.members.is_empty());
}
