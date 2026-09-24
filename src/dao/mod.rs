//! 数据库适配层；对 trace 调用者隐藏四种后端的差异。

mod mongo;
mod sql;

use std::collections::{HashMap, HashSet};

use crate::{
    config::{TraceDatabase, TraceDatabaseConfig},
    trace::{
        BatchWriteItem, TraceCursor, TraceError, TracePage, TraceReader, TraceRecord, TraceWriter,
    },
};

pub(crate) enum TraceStore {
    Sql(sql::SqlStore),
    Mongo(mongo::MongoStore),
}

impl TraceStore {
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
