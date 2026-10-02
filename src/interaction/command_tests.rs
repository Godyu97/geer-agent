use std::{cell::RefCell, io, rc::Rc};

use super::{CommandOutcome, Operation, execute, save};
use crate::interaction::{Input, Session, SessionScope, test_support::MockSession};

#[tokio::test]
async fn memory_management_never_calls_the_model_and_confirmation_is_separate() {
    use crate::{
        config::{TraceDatabase, TraceDatabaseConfig},
        dao::MemoryStore,
        interaction::{MemoryCommand, parse_input},
        memory::{MemoryAction, MemoryService},
    };
    let mut session = MockSession {
        memory: MemoryService::new(
            MemoryStore::connect(&TraceDatabaseConfig {
                kind: TraceDatabase::Sqlite,
                url: "sqlite::memory:".into(),
            })
            .await
            .unwrap(),
        )
        .await
        .unwrap(),
        ..Default::default()
    };
    let command = |line: &str| parse_input(line);
    execute(
        &mut session,
        command("/memory add useful fact"),
        |_| panic!("不调用模型"),
        |_| panic!("不调用模型"),
    )
    .await
    .unwrap();
    let id = session.memories().await.unwrap()[0].id.clone();
    let outcome = execute(
        &mut session,
        command("/memory search USEFUL"),
        |_| Ok(()),
        |_| Ok(()),
    )
    .await
    .unwrap();
    assert!(matches!(outcome, CommandOutcome::Memories { entries, .. } if entries.len() == 1));
    execute(
        &mut session,
        command(&format!("/memory edit {id} edited fact")),
        |_| Ok(()),
        |_| Ok(()),
    )
    .await
    .unwrap();
    let preview = execute(
        &mut session,
        command("/memory clear"),
        |_| Ok(()),
        |_| Ok(()),
    )
    .await
    .unwrap();
    assert!(
        matches!(preview, CommandOutcome::MemoryPreview(preview) if preview.action == MemoryAction::Clear && preview.count == 1)
    );
    assert_eq!(session.memories().await.unwrap().len(), 1);
    execute(
        &mut session,
        Input::Memory(MemoryCommand::Clear { confirmed: true }),
        |_| Ok(()),
        |_| Ok(()),
    )
    .await
    .unwrap();
    assert!(session.memories().await.unwrap().is_empty());
    assert!(session.seen.is_empty());
}

#[tokio::test]
async fn forwards_deltas_and_usage_in_order_and_preserves_partial_failure() {
    let mut session = MockSession {
        fail: true,
        ..Default::default()
    };
    let events = Rc::new(RefCell::new(Vec::new()));
    let deltas = Rc::clone(&events);
    let usage = Rc::clone(&events);
    let error = execute(
        &mut session,
        Input::Message("hello".into()),
        move |text| {
            deltas.borrow_mut().push(format!("delta:{text}"));
            Ok(())
        },
        move |tokens| {
            usage
                .borrow_mut()
                .push(format!("usage:{}", tokens.unwrap().output));
            Ok(())
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.operation, Operation::Message);
    assert_eq!(error.message, "mock failure");
    assert_eq!(*events.borrow(), ["delta:你", "usage:4", "delta:好"]);
    assert_eq!(session.seen, ["hello"]);
}

#[tokio::test]
async fn callback_failure_stops_delivery_and_reaches_the_caller() {
    let mut session = MockSession::default();
    let error = execute(
        &mut session,
        Input::Message("hello".into()),
        |_| Err(io::Error::new(io::ErrorKind::BrokenPipe, "sink closed")),
        |_| panic!("首个正文回调失败后不应继续报告用量"),
    )
    .await
    .unwrap_err();
    assert_eq!(error.operation, Operation::Message);
    assert_eq!(error.message, "sink closed");
}

#[tokio::test]
async fn session_commands_do_not_enter_conversation_and_keep_scope_and_identity() {
    let mut session = MockSession::default();
    for reset in [false, true] {
        let outcome = execute(
            &mut session,
            if reset { Input::Reset } else { Input::New },
            |_| panic!("命令不发送模型正文"),
            |_| panic!("命令不发送模型用量"),
        )
        .await
        .unwrap();
        assert!(
            matches!(outcome, CommandOutcome::NewSession { session_id, reset: actual } if session_id == "new-session" && actual == reset)
        );
    }
    let outcome = execute(
        &mut session,
        Input::Open("saved".into()),
        |_| Ok(()),
        |_| Ok(()),
    )
    .await
    .unwrap();
    assert!(matches!(outcome, CommandOutcome::Opened { session_id, .. } if session_id == "saved"));
    execute(
        &mut session,
        Input::Workspace(Some("/tmp/other space".into())),
        |_| Ok(()),
        |_| Ok(()),
    )
    .await
    .unwrap();
    assert_eq!(session.workspace(), "/tmp/other space");
    for scope in [SessionScope::Current, SessionScope::All] {
        execute(&mut session, Input::Sessions(scope), |_| Ok(()), |_| Ok(()))
            .await
            .unwrap();
    }
    assert_eq!(
        *session.scopes.borrow(),
        [SessionScope::Current, SessionScope::All]
    );
    assert!(session.seen.is_empty());
}

#[tokio::test]
async fn preview_does_not_delete_and_direct_confirmation_validates_exact_ids() {
    let id = "11111111-1111-4111-8111-111111111111";
    let mut session = MockSession::default();
    let outcome = execute(
        &mut session,
        Input::Delete {
            ids: vec![id.into(), id.into()],
            confirmed: false,
        },
        |_| Ok(()),
        |_| Ok(()),
    )
    .await
    .unwrap();
    assert!(matches!(outcome, CommandOutcome::DeletePreview(preview) if preview.ids() == vec![id]));
    assert!(session.seen.is_empty());
    execute(
        &mut session,
        Input::Delete {
            ids: vec![id.into(), id.into()],
            confirmed: true,
        },
        |_| Ok(()),
        |_| Ok(()),
    )
    .await
    .unwrap();
    assert_eq!(session.seen, [id]);
    for ids in [vec![], vec!["wrong".into()], vec![id.into(), "*".into()]] {
        let error = execute(
            &mut session,
            Input::Delete {
                ids,
                confirmed: true,
            },
            |_| Ok(()),
            |_| Ok(()),
        )
        .await
        .unwrap_err();
        assert_eq!(error.operation, Operation::Input);
        assert_eq!(session.seen, [id]);
    }
}

#[tokio::test]
async fn failed_open_preserves_session_and_identifies_operation() {
    let mut session = MockSession {
        fail_open: true,
        ..Default::default()
    };
    let error = execute(
        &mut session,
        Input::Open("saved".into()),
        |_| Ok(()),
        |_| Ok(()),
    )
    .await
    .unwrap_err();
    assert_eq!(error.operation, Operation::Open);
    assert_eq!(session.session_id(), "mock-session");
    assert!(session.actions.is_empty());
}

#[tokio::test]
async fn exit_is_a_request_and_lifecycle_save_uses_the_same_operation() {
    let mut session = MockSession::default();
    let outcome = execute(&mut session, Input::Exit, |_| Ok(()), |_| Ok(()))
        .await
        .unwrap();
    assert!(matches!(outcome, CommandOutcome::Exit));
    assert!(session.actions.is_empty());
    assert_eq!(save(&mut session).await, "已保存");
    let outcome = execute(&mut session, Input::Save, |_| Ok(()), |_| Ok(()))
        .await
        .unwrap();
    assert!(matches!(outcome, CommandOutcome::Saved(report) if report == "已保存"));
    assert_eq!(session.actions, ["save", "save"]);
}
