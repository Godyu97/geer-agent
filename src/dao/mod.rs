//! 数据库适配层；对 trace 调用者隐藏四种后端的差异。

mod mongo;
mod sql;

pub(crate) const SESSION_REVISION_CONFLICT: &str = "会话 revision 冲突";

use std::collections::{HashMap, HashSet};

use crate::{
    config::{TraceDatabase, TraceDatabaseConfig},
    memory::MemoryEntry,
    session::{SessionEvent, SessionRecord, StoreDeletion},
    trace::{
        BatchWriteItem, TraceCursor, TraceError, TracePage, TraceReader, TraceRecord, TraceWriter,
    },
};

#[derive(Clone)]
pub(crate) enum SessionStore {
    Sql(sql::SqlStore),
    Mongo(mongo::MongoStore),
}

impl SessionStore {
    pub(crate) fn memory_store(&self) -> MemoryStore {
        match self {
            Self::Sql(store) => MemoryStore::Sql(store.clone()),
            Self::Mongo(store) => MemoryStore::Mongo(store.clone()),
        }
    }

    pub(crate) async fn delete(&self, id: &str) -> Result<StoreDeletion, TraceError> {
        match self {
            Self::Sql(store) => store.delete_session(id).await,
            Self::Mongo(store) => store.delete_session(id).await,
        }
    }

    pub(crate) fn trace_store(&self) -> TraceStore {
        match self {
            Self::Sql(store) => TraceStore::Sql(store.clone()),
            Self::Mongo(store) => TraceStore::Mongo(store.clone()),
        }
    }
    pub(crate) async fn connect(config: &TraceDatabaseConfig) -> Result<Self, TraceError> {
        match config.kind {
            TraceDatabase::Sqlite | TraceDatabase::Postgres | TraceDatabase::Mysql => {
                sql::SqlStore::connect(&config.url).await.map(Self::Sql)
            }
            TraceDatabase::Mongodb => mongo::MongoStore::connect(&config.url)
                .await
                .map(Self::Mongo),
        }
    }

    pub(crate) async fn save(
        &self,
        record: &SessionRecord,
        events: &[SessionEvent],
        expected_revision: Option<i64>,
    ) -> Result<(), TraceError> {
        match self {
            Self::Sql(store) => store.save_session(record, events, expected_revision).await,
            Self::Mongo(store) => store.save_session(record, events, expected_revision).await,
        }
    }

    pub(crate) async fn load(&self, id: &str) -> Result<Option<SessionRecord>, TraceError> {
        match self {
            Self::Sql(store) => store.load_session(id).await,
            Self::Mongo(store) => store.load_session(id).await,
        }
    }

    pub(crate) async fn list(&self, workspace: &str) -> Result<Vec<SessionRecord>, TraceError> {
        match self {
            Self::Sql(store) => store.list_sessions(workspace).await,
            Self::Mongo(store) => store.list_sessions(workspace).await,
        }
    }

    pub(crate) async fn list_all(&self) -> Result<Vec<SessionRecord>, TraceError> {
        match self {
            Self::Sql(store) => store.list_all_sessions().await,
            Self::Mongo(store) => store.list_all_sessions().await,
        }
    }

    pub(crate) async fn history(
        &self,
        record: &SessionRecord,
    ) -> Result<Vec<SessionEvent>, TraceError> {
        let mut next = record.head_event_id.clone();
        let mut seen = HashSet::new();
        let mut reversed = Vec::new();
        while let Some(id) = next {
            if !seen.insert(id.clone()) {
                return Err(TraceError("会话事件链存在循环".into()));
            }
            let event = match self {
                Self::Sql(store) => store.load_session_event(&id).await?,
                Self::Mongo(store) => store.load_session_event(&id).await?,
            }
            .ok_or_else(|| TraceError("会话事件链缺失".into()))?;
            if event.session_id != record.id {
                return Err(TraceError("会话事件关联错误".into()));
            }
            next = event.parent_id.clone();
            reversed.push(event);
        }
        reversed.reverse();
        Ok(reversed)
    }
}

pub(crate) enum TraceStore {
    Sql(sql::SqlStore),
    Mongo(mongo::MongoStore),
}

#[derive(Clone)]
pub(crate) enum MemoryStore {
    Sql(sql::SqlStore),
    Mongo(mongo::MongoStore),
}

impl MemoryStore {
    pub(crate) async fn connect(config: &TraceDatabaseConfig) -> Result<Self, TraceError> {
        SessionStore::connect(config)
            .await
            .map(|store| store.memory_store())
    }

    pub(crate) async fn list(&self) -> Result<Vec<MemoryEntry>, TraceError> {
        match self {
            Self::Sql(store) => store.list_memories().await,
            Self::Mongo(store) => store.list_memories().await,
        }
    }

    pub(crate) async fn insert(&self, entry: &MemoryEntry) -> Result<(), TraceError> {
        match self {
            Self::Sql(store) => store.insert_memory(entry).await,
            Self::Mongo(store) => store.insert_memory(entry).await,
        }
    }

    pub(crate) async fn update(&self, entry: &MemoryEntry) -> Result<bool, TraceError> {
        match self {
            Self::Sql(store) => store.update_memory(entry).await,
            Self::Mongo(store) => store.update_memory(entry).await,
        }
    }

    pub(crate) async fn delete(&self, id: &str) -> Result<bool, TraceError> {
        match self {
            Self::Sql(store) => store.delete_memory(id).await,
            Self::Mongo(store) => store.delete_memory(id).await,
        }
    }

    pub(crate) async fn clear(&self) -> Result<u64, TraceError> {
        match self {
            Self::Sql(store) => store.clear_memories().await,
            Self::Mongo(store) => store.clear_memories().await,
        }
    }
}

impl TraceStore {
    pub(crate) fn memory_store(&self) -> MemoryStore {
        match self {
            Self::Sql(store) => MemoryStore::Sql(store.clone()),
            Self::Mongo(store) => MemoryStore::Mongo(store.clone()),
        }
    }

    pub(crate) async fn connect(config: &TraceDatabaseConfig) -> Result<Self, TraceError> {
        match config.kind {
            TraceDatabase::Sqlite | TraceDatabase::Postgres | TraceDatabase::Mysql => {
                sql::SqlStore::connect(&config.url).await.map(Self::Sql)
            }
            TraceDatabase::Mongodb => mongo::MongoStore::connect(&config.url)
                .await
                .map(Self::Mongo),
        }
    }

    async fn insert_one(&self, record: &TraceRecord) -> Result<(), TraceError> {
        match self {
            Self::Sql(store) => store.insert_one(record).await,
            Self::Mongo(store) => store.insert_one(record).await,
        }
    }

    async fn insert_many(&self, records: &[TraceRecord]) -> Result<(), TraceError> {
        for chunk in records.chunks(50) {
            match self {
                Self::Sql(store) => store.insert_many(chunk).await?,
                Self::Mongo(store) => store.insert_many(chunk).await?,
            }
        }
        Ok(())
    }
}

impl TraceWriter for TraceStore {
    async fn write_one(&self, record: &TraceRecord) -> Result<(), TraceError> {
        if let Some(existing) = self.get_one(&record.request_id).await? {
            return if existing == *record {
                Ok(())
            } else {
                Err(TraceError("Trace request_id 冲突".into()))
            };
        }
        match self.insert_one(record).await {
            Ok(()) => Ok(()),
            Err(error) => match self.get_one(&record.request_id).await? {
                Some(existing) if existing == *record => Ok(()),
                Some(_) => Err(TraceError("Trace request_id 冲突".into())),
                None => Err(error),
            },
        }
    }

    async fn write_batch(
        &self,
        records: &[TraceRecord],
    ) -> Result<Vec<BatchWriteItem>, TraceError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }
        let mut unique = HashMap::<&str, &TraceRecord>::new();
        let mut conflicts = HashSet::<&str>::new();
        for record in records {
            match unique.get(record.request_id.as_str()) {
                Some(existing) if **existing != *record => {
                    conflicts.insert(record.request_id.as_str());
                }
                _ => {
                    unique.entry(record.request_id.as_str()).or_insert(record);
                }
            }
        }
        let candidates: Vec<_> = unique
            .into_values()
            .filter(|record| !conflicts.contains(record.request_id.as_str()))
            .collect();
        let ids: Vec<_> = candidates
            .iter()
            .map(|record| record.request_id.clone())
            .collect();
        let existing = self.get_batch(&ids).await?;
        let mut outcomes = HashMap::<&str, Result<(), TraceError>>::new();
        let mut pending = Vec::new();
        for (record, stored) in candidates.into_iter().zip(existing) {
            match stored {
                Some(stored) if stored == *record => {
                    outcomes.insert(&record.request_id, Ok(()));
                }
                Some(_) => {
                    outcomes.insert(
                        &record.request_id,
                        Err(TraceError("Trace request_id 冲突".into())),
                    );
                }
                None => pending.push(record.clone()),
            }
        }
        if !pending.is_empty() {
            let bulk_result = self.insert_many(&pending).await;
            let pending_ids: Vec<_> = pending
                .iter()
                .map(|record| record.request_id.clone())
                .collect();
            let after = self.get_batch(&pending_ids).await?;
            for (record, stored) in pending.iter().zip(after) {
                let result = match stored {
                    Some(stored) if stored == *record => Ok(()),
                    Some(_) => Err(TraceError("Trace request_id 冲突".into())),
                    None if bulk_result.is_err() => {
                        let insert_result = self.insert_one(record).await;
                        match self.get_one(&record.request_id).await {
                            Ok(Some(stored)) if stored == *record => Ok(()),
                            Ok(Some(_)) => Err(TraceError("Trace request_id 冲突".into())),
                            Ok(None) => Err(insert_result
                                .err()
                                .unwrap_or_else(|| TraceError("Trace 补写后记录缺失".into()))),
                            Err(_) => return Err(TraceError("Trace 批次写入状态无法确认".into())),
                        }
                    }
                    None => Err(TraceError("Trace 批量写入后记录缺失".into())),
                };
                outcomes.insert(&record.request_id, result);
            }
        }
        Ok(records
            .iter()
            .map(|record| BatchWriteItem {
                request_id: record.request_id.clone(),
                result: if conflicts.contains(record.request_id.as_str()) {
                    Err(TraceError("批次内 request_id 冲突".into()))
                } else {
                    outcomes
                        .get(record.request_id.as_str())
                        .cloned()
                        .unwrap_or_else(|| Err(TraceError("批次结果未知".into())))
                },
            })
            .collect())
    }
}

impl TraceReader for TraceStore {
    async fn get_one(&self, request_id: &str) -> Result<Option<TraceRecord>, TraceError> {
        match self {
            Self::Sql(store) => store.get_one(request_id).await,
            Self::Mongo(store) => store.get_one(request_id).await,
        }
    }

    async fn get_batch(
        &self,
        request_ids: &[String],
    ) -> Result<Vec<Option<TraceRecord>>, TraceError> {
        let mut result = Vec::with_capacity(request_ids.len());
        for chunk in request_ids.chunks(100) {
            result.extend(match self {
                Self::Sql(store) => store.get_batch(chunk).await?,
                Self::Mongo(store) => store.get_batch(chunk).await?,
            });
        }
        Ok(result)
    }

    async fn list_session_page(
        &self,
        session_id: &str,
        after: Option<&TraceCursor>,
        limit: u64,
    ) -> Result<TracePage, TraceError> {
        if limit == 0 {
            return Ok(TracePage {
                items: Vec::new(),
                next_cursor: None,
            });
        }
        match self {
            Self::Sql(store) => store.list_session_page(session_id, after, limit).await,
            Self::Mongo(store) => store.list_session_page(session_id, after, limit).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::TraceStatus;
    use serde_json::json;
    use uuid::Uuid;

    use crate::session::{SessionEvent, SessionRecord};

    fn record(id: &str, session: &str, time: i64) -> TraceRecord {
        TraceRecord {
            request_id: id.into(),
            session_id: session.into(),
            agent_run_id: "run-1".into(),
            provider_response_id: Some("response-1".into()),
            api: "responses".into(),
            model: "test".into(),
            started_at_ms: time,
            duration_ms: 12,
            attempts: 2,
            status: TraceStatus::Completed,
            input_tokens: Some(3),
            output_tokens: Some(4),
            request: json!({"input": "secret full input"}),
            response: Some(json!({"output": "full reply"})),
            error: None,
        }
    }

    async fn contract(config: TraceDatabaseConfig) {
        let store = TraceStore::connect(&config)
            .await
            .expect("数据库连接和建表");
        let session = Uuid::new_v4().to_string();
        let first = record(&Uuid::new_v4().to_string(), &session, 100);
        let second = record(&Uuid::new_v4().to_string(), &session, 100);
        let third = record(&Uuid::new_v4().to_string(), &session, 101);
        store.write_one(&first).await.expect("单条写入");
        store.write_one(&first).await.expect("幂等单条写入");
        assert_eq!(
            store.get_one(&first.request_id).await.unwrap(),
            Some(first.clone())
        );
        assert_eq!(store.get_one("missing").await.unwrap(), None);
        assert!(store.write_batch(&[]).await.unwrap().is_empty());
        let results = store
            .write_batch(&[second.clone(), third.clone(), first.clone()])
            .await
            .unwrap();
        assert!(results.iter().all(|item| item.result.is_ok()));
        assert_eq!(
            results
                .iter()
                .map(|item| item.request_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                second.request_id.as_str(),
                third.request_id.as_str(),
                first.request_id.as_str()
            ]
        );
        let mut altered = second.clone();
        altered.request = json!({"different": true});
        let results = store
            .write_batch(&[second.clone(), altered, third.clone()])
            .await
            .unwrap();
        assert!(
            results[0].result.is_err() && results[1].result.is_err() && results[2].result.is_ok()
        );
        assert_eq!(
            store
                .get_batch(&[
                    third.request_id.clone(),
                    "missing".into(),
                    first.request_id.clone(),
                    third.request_id.clone()
                ])
                .await
                .unwrap(),
            vec![
                Some(third.clone()),
                None,
                Some(first.clone()),
                Some(third.clone())
            ]
        );
        let page = store.list_session_page(&session, None, 2).await.unwrap();
        assert_eq!(page.items.len(), 2);
        let next = store
            .list_session_page(&session, page.next_cursor.as_ref(), 2)
            .await
            .unwrap();
        assert_eq!(next.items, vec![third]);
        assert!(next.next_cursor.is_none());
    }

    async fn session_contract(config: TraceDatabaseConfig) {
        let store = SessionStore::connect(&config)
            .await
            .expect("会话数据库连接和迁移");
        let id = Uuid::new_v4().to_string();
        let event_id = Uuid::new_v4().to_string();
        let event = SessionEvent {
            id: event_id.clone(),
            parent_id: None,
            session_id: id.clone(),
            kind: "user".into(),
            payload: json!({"text": "first"}),
        };
        let first = SessionRecord {
            id: id.clone(),
            workspace: "/tmp/workspace-a".into(),
            api: "chat-completions".into(),
            model: "test".into(),
            endpoint: "https://example.test/v1".into(),
            snapshot: json!({"version": 1, "messages": ["first"]}),
            head_event_id: Some(event_id),
            revision: 0,
            updated_at_ms: 100,
            uncertain_tools: false,
        };
        store
            .save(&first, std::slice::from_ref(&event), None)
            .await
            .unwrap();
        assert_eq!(store.load(&id).await.unwrap(), Some(first.clone()));
        assert_eq!(store.history(&first).await.unwrap(), vec![event.clone()]);
        let listed = store.list("/tmp/workspace-a").await.unwrap();
        assert!(listed.iter().any(|item| item.id == id));
        assert!(
            !store
                .list("/tmp/workspace-b")
                .await
                .unwrap()
                .iter()
                .any(|item| item.id == id)
        );
        let mut other = first.clone();
        other.id = Uuid::new_v4().to_string();
        other.workspace = "/tmp/workspace-b".into();
        other.head_event_id = None;
        other.updated_at_ms = 200;
        store.save(&other, &[], None).await.unwrap();
        assert!(
            store
                .list("/tmp/workspace-b")
                .await
                .unwrap()
                .iter()
                .any(|item| item.id == other.id)
        );
        let all = store.list_all().await.unwrap();
        assert!(all.iter().any(|item| item.id == id));
        assert!(all.iter().any(|item| item.id == other.id));
        let mut newer = first.clone();
        newer.revision = 1;
        newer.updated_at_ms = 101;
        newer.uncertain_tools = true;
        store.save(&newer, &[], Some(0)).await.unwrap();
        assert!(store.save(&first, &[], Some(0)).await.is_err());
        let orphan = SessionEvent {
            id: Uuid::new_v4().to_string(),
            parent_id: newer.head_event_id.clone(),
            session_id: id.clone(),
            kind: "user".into(),
            payload: json!({"text": "unpublished"}),
        };
        let mut stale = newer.clone();
        stale.revision = 2;
        stale.head_event_id = Some(orphan.id.clone());
        assert!(store.save(&stale, &[orphan], Some(0)).await.is_err());
        assert_eq!(store.history(&newer).await.unwrap(), vec![event]);
        assert_eq!(store.load(&id).await.unwrap(), Some(newer.clone()));
        let trace = record(&Uuid::new_v4().to_string(), &id, 102);
        let traces = store.trace_store();
        traces.write_one(&trace).await.unwrap();
        let deleted = store.delete(&id).await.unwrap();
        assert!(deleted.existed);
        assert!(deleted.cleanup_error.is_none());
        assert!(store.load(&id).await.unwrap().is_none());
        assert!(store.history(&newer).await.is_err());
        assert!(store.history(&stale).await.is_err());
        assert!(store.load(&other.id).await.unwrap().is_some());
        assert_eq!(
            traces.get_one(&trace.request_id).await.unwrap(),
            Some(trace)
        );
        assert!(store.save(&stale, &[], Some(1)).await.is_err());
        assert!(store.load(&id).await.unwrap().is_none());
        let repeated = store.delete(&id).await.unwrap();
        assert!(!repeated.existed);
        assert!(repeated.cleanup_error.is_none());
    }

    #[tokio::test]
    async fn sqlite_session_contract() {
        let path = std::env::temp_dir().join(format!("geer-session-{}.sqlite", Uuid::new_v4()));
        session_contract(TraceDatabaseConfig {
            kind: TraceDatabase::Sqlite,
            url: format!("sqlite://{}?mode=rwc", path.display()),
        })
        .await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn sqlite_shared_connection_creates_parent_and_holds_both_records() {
        let root = std::env::temp_dir().join(format!("geer-shared-{}", Uuid::new_v4()));
        let path = root.join("nested/geer.sqlite");
        let config = TraceDatabaseConfig {
            kind: TraceDatabase::Sqlite,
            url: format!("sqlite://{}?mode=rwc", path.display()),
        };
        let session_store = SessionStore::connect(&config).await.unwrap();
        let trace_store = session_store.trace_store();
        assert!(path.is_file());
        let id = Uuid::new_v4().to_string();
        let session = SessionRecord {
            id: id.clone(),
            workspace: "/tmp/shared".into(),
            api: "responses".into(),
            model: "test".into(),
            endpoint: "https://example.test/v1".into(),
            snapshot: json!({"version":1}),
            head_event_id: None,
            revision: 0,
            updated_at_ms: 100,
            uncertain_tools: false,
        };
        session_store.save(&session, &[], None).await.unwrap();
        let trace = record(&Uuid::new_v4().to_string(), &id, 101);
        trace_store.write_one(&trace).await.unwrap();
        assert_eq!(session_store.load(&id).await.unwrap(), Some(session));
        assert_eq!(
            trace_store.get_one(&trace.request_id).await.unwrap(),
            Some(trace)
        );
        drop(trace_store);
        drop(session_store);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn external_session_contracts() {
        for (key, kind) in [
            ("GEER_AGENT_TEST_POSTGRES_URL", TraceDatabase::Postgres),
            ("GEER_AGENT_TEST_MYSQL_URL", TraceDatabase::Mysql),
            ("GEER_AGENT_TEST_MONGODB_URL", TraceDatabase::Mongodb),
        ] {
            if let Ok(url) = std::env::var(key) {
                session_contract(TraceDatabaseConfig { kind, url }).await;
            }
        }
    }

    #[tokio::test]
    async fn sqlite_contract() {
        let path = std::env::temp_dir().join(format!("geer-trace-{}.sqlite", Uuid::new_v4()));
        contract(TraceDatabaseConfig {
            kind: TraceDatabase::Sqlite,
            url: format!("sqlite://{}?mode=rwc", path.display()),
        })
        .await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn external_contracts() {
        for (key, kind) in [
            ("GEER_AGENT_TEST_POSTGRES_URL", TraceDatabase::Postgres),
            ("GEER_AGENT_TEST_MYSQL_URL", TraceDatabase::Mysql),
            ("GEER_AGENT_TEST_MONGODB_URL", TraceDatabase::Mongodb),
        ] {
            if let Ok(url) = std::env::var(key) {
                contract(TraceDatabaseConfig { kind, url }).await;
            }
        }
    }

    #[tokio::test]
    async fn mongodb_oversized_batch_reports_each_confirmed_result() {
        let Ok(url) = std::env::var("GEER_AGENT_TEST_MONGODB_URL") else {
            return;
        };
        let store = TraceStore::connect(&TraceDatabaseConfig {
            kind: TraceDatabase::Mongodb,
            url,
        })
        .await
        .unwrap();
        let session = Uuid::new_v4().to_string();
        let small = record(&Uuid::new_v4().to_string(), &session, 1);
        let mut large = record(&Uuid::new_v4().to_string(), &session, 2);
        large.request = json!({"input": "x".repeat(17 * 1024 * 1024)});
        let results = store
            .write_batch(&[small.clone(), large.clone()])
            .await
            .unwrap();
        assert!(results[0].result.is_ok());
        assert!(results[1].result.is_err());
        assert_eq!(store.get_one(&small.request_id).await.unwrap(), Some(small));
        assert_eq!(store.get_one(&large.request_id).await.unwrap(), None);
    }
}
