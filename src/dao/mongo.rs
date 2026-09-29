use std::time::Duration;

use futures_util::TryStreamExt;
use mongodb::{Client, Collection, IndexModel, bson::doc, options::ClientOptions};
use serde::{Deserialize, Serialize};

use crate::{
    session::{SessionEvent, SessionRecord},
    trace::{TraceCursor, TraceError, TracePage, TraceRecord},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TraceDocument {
    #[serde(rename = "_id")]
    id: String,
    #[serde(flatten)]
    record: TraceRecord,
}

impl From<&TraceRecord> for TraceDocument {
    fn from(record: &TraceRecord) -> Self {
        Self {
            id: record.request_id.clone(),
            record: record.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SessionDocument {
    #[serde(rename = "_id")]
    id: String,
    record: SessionRecord,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SessionEventDocument {
    #[serde(rename = "_id")]
    id: String,
    event: SessionEvent,
}

#[derive(Clone)]
pub(crate) struct MongoStore {
    collection: Collection<TraceDocument>,
    sessions: Collection<SessionDocument>,
    session_events: Collection<SessionEventDocument>,
}

impl MongoStore {
    pub(super) async fn connect(url: &str) -> Result<Self, TraceError> {
        let mut options = ClientOptions::parse(url)
            .await
            .map_err(|_| TraceError("MongoDB URI 无效".into()))?;
        options.server_selection_timeout = Some(Duration::from_secs(5));
        let client =
            Client::with_options(options).map_err(|_| TraceError("MongoDB 初始化失败".into()))?;
        let database = client
            .default_database()
            .ok_or_else(|| TraceError("MongoDB URI 缺少数据库名".into()))?;
        database
            .run_command(doc! {"ping": 1})
            .await
            .map_err(|_| TraceError("MongoDB 连接失败".into()))?;
        let collection = database.collection::<TraceDocument>("llm_traces");
        collection
            .create_index(
                IndexModel::builder()
                    .keys(doc! {"session_id": 1, "started_at_ms": 1, "request_id": 1})
                    .build(),
            )
            .await
            .map_err(|_| TraceError("MongoDB 索引创建失败".into()))?;
        let sessions = database.collection::<SessionDocument>("agent_sessions");
        sessions
            .create_index(
                IndexModel::builder()
                    .keys(doc! {"record.workspace": 1, "record.updated_at_ms": -1})
                    .build(),
            )
            .await
            .map_err(|_| TraceError("MongoDB 会话索引创建失败".into()))?;
        let session_events = database.collection::<SessionEventDocument>("agent_session_events");
        session_events
            .create_index(
                IndexModel::builder()
                    .keys(doc! {"event.session_id": 1})
                    .build(),
            )
            .await
            .map_err(|_| TraceError("MongoDB 会话事件索引创建失败".into()))?;
        Ok(Self {
            collection,
            sessions,
            session_events,
        })
    }

    pub(super) async fn save_session(
        &self,
        record: &SessionRecord,
        events: &[SessionEvent],
        expected_revision: Option<i64>,
    ) -> Result<(), TraceError> {
        for event in events {
            let doc = SessionEventDocument {
                id: event.id.clone(),
                event: event.clone(),
            };
            if self.session_events.insert_one(&doc).await.is_err() {
                let stored = self
                    .session_events
                    .find_one(doc! {"_id": &event.id})
                    .await
                    .map_err(|_| TraceError("MongoDB 会话事件读取失败".into()))?;
                if stored != Some(doc) {
                    return Err(TraceError("MongoDB 会话事件写入冲突".into()));
                }
            }
        }
        let doc = SessionDocument {
            id: record.id.clone(),
            record: record.clone(),
        };
        if let Some(expected) = expected_revision {
            if record.revision != expected + 1 {
                return Err(TraceError("会话 revision 无效".into()));
            }
            let outcome = self
                .sessions
                .replace_one(doc! {"_id": &record.id, "record.revision": expected}, doc)
                .await
                .map_err(|_| TraceError("MongoDB 会话检查点更新失败".into()))?;
            if outcome.matched_count != 1 {
                return Err(TraceError(super::SESSION_REVISION_CONFLICT.into()));
            }
        } else {
            if record.revision != 0 {
                return Err(TraceError("初始会话 revision 无效".into()));
            }
            self.sessions
                .insert_one(doc)
                .await
                .map_err(|_| TraceError("MongoDB 会话检查点创建失败或 ID 冲突".into()))?;
        }
        Ok(())
    }

    pub(super) async fn load_session(&self, id: &str) -> Result<Option<SessionRecord>, TraceError> {
        self.sessions
            .find_one(doc! {"_id": id})
            .await
            .map(|doc| doc.map(|doc| doc.record))
            .map_err(|_| TraceError("MongoDB 会话读取失败".into()))
    }

    pub(super) async fn load_session_event(
        &self,
        id: &str,
    ) -> Result<Option<SessionEvent>, TraceError> {
        self.session_events
            .find_one(doc! {"_id": id})
            .await
            .map(|doc| doc.map(|doc| doc.event))
            .map_err(|_| TraceError("MongoDB 会话事件读取失败".into()))
    }

    pub(super) async fn list_sessions(
        &self,
        workspace: &str,
    ) -> Result<Vec<SessionRecord>, TraceError> {
        let docs: Vec<SessionDocument> = self
            .sessions
            .find(doc! {"record.workspace": workspace})
            .sort(doc! {"record.updated_at_ms": -1, "_id": -1})
            .limit(20)
            .await
            .map_err(|_| TraceError("MongoDB 会话列表读取失败".into()))?
            .try_collect()
            .await
            .map_err(|_| TraceError("MongoDB 会话列表读取失败".into()))?;
        Ok(docs.into_iter().map(|doc| doc.record).collect())
    }

    pub(super) async fn list_all_sessions(&self) -> Result<Vec<SessionRecord>, TraceError> {
        let docs: Vec<SessionDocument> = self
            .sessions
            .find(doc! {})
            .sort(doc! {"record.updated_at_ms": -1, "_id": -1})
            .limit(20)
            .await
            .map_err(|_| TraceError("MongoDB 会话列表读取失败".into()))?
            .try_collect()
            .await
            .map_err(|_| TraceError("MongoDB 会话列表读取失败".into()))?;
        Ok(docs.into_iter().map(|doc| doc.record).collect())
    }

    pub(super) async fn insert_one(&self, record: &TraceRecord) -> Result<(), TraceError> {
        self.collection
            .insert_one(TraceDocument::from(record))
            .await
            .map(|_| ())
            .map_err(|_| TraceError("MongoDB 写入失败".into()))
    }

    pub(super) async fn insert_many(&self, records: &[TraceRecord]) -> Result<(), TraceError> {
        if records.is_empty() {
            return Ok(());
        }
        self.collection
            .insert_many(records.iter().map(TraceDocument::from).collect::<Vec<_>>())
            .ordered(false)
            .await
            .map(|_| ())
            .map_err(|_| TraceError("MongoDB 批量写入失败".into()))
    }

    pub(super) async fn get_one(
        &self,
        request_id: &str,
    ) -> Result<Option<TraceRecord>, TraceError> {
        self.collection
            .find_one(doc! {"_id": request_id})
            .await
            .map(|record| record.map(|record| record.record))
            .map_err(|_| TraceError("MongoDB 读取失败".into()))
    }

    pub(super) async fn get_batch(
        &self,
        request_ids: &[String],
    ) -> Result<Vec<Option<TraceRecord>>, TraceError> {
        if request_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<TraceDocument> = self
            .collection
            .find(doc! {"_id": {"$in": request_ids.to_vec()}})
            .await
            .map_err(|_| TraceError("MongoDB 批量读取失败".into()))?
            .try_collect()
            .await
            .map_err(|_| TraceError("MongoDB 批量读取失败".into()))?;
        let by_id: std::collections::HashMap<_, _> =
            rows.into_iter().map(|row| (row.id, row.record)).collect();
        Ok(request_ids
            .iter()
            .map(|id| by_id.get(id).cloned())
            .collect())
    }

    pub(super) async fn list_session_page(
        &self,
        session_id: &str,
        after: Option<&TraceCursor>,
        limit: u64,
    ) -> Result<TracePage, TraceError> {
        let mut filter = doc! {"session_id": session_id};
        if let Some(cursor) = after {
            filter.insert("$or", vec![
                doc! {"started_at_ms": {"$gt": cursor.started_at_ms}},
                doc! {"started_at_ms": cursor.started_at_ms, "request_id": {"$gt": &cursor.request_id}},
            ]);
        }
        let page_size = limit.min(100);
        let rows: Vec<TraceDocument> = self
            .collection
            .find(filter)
            .sort(doc! {"started_at_ms": 1, "request_id": 1})
            .limit(i64::try_from(page_size + 1).unwrap_or(101))
            .await
            .map_err(|_| TraceError("MongoDB 分页读取失败".into()))?
            .try_collect()
            .await
            .map_err(|_| TraceError("MongoDB 分页读取失败".into()))?;
        let has_more = rows.len() as u64 > page_size;
        let items: Vec<_> = rows
            .into_iter()
            .take(page_size as usize)
            .map(|row| row.record)
            .collect();
        let next_cursor = if has_more {
            items.last().map(|item| TraceCursor {
                started_at_ms: item.started_at_ms,
                request_id: item.request_id.clone(),
            })
        } else {
            None
        };
        Ok(TracePage { items, next_cursor })
    }
}
