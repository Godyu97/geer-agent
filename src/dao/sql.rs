use std::{str::FromStr, time::Duration};

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectOptions, Database, DatabaseConnection,
    DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
    sea_query::Condition, sqlx::sqlite::SqliteConnectOptions,
};
use sea_orm_migration::prelude::*;

use crate::session::{SessionEvent, SessionRecord, StoreDeletion};
use crate::trace::{TraceCursor, TraceError, TracePage, TraceRecord, TraceStatus};

mod entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "llm_traces")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub request_id: String,
        pub session_id: String,
        pub agent_run_id: String,
        pub provider_response_id: Option<String>,
        pub api: String,
        #[sea_orm(column_type = "Text")]
        pub model: String,
        pub started_at_ms: i64,
        pub duration_ms: i64,
        pub attempts: i32,
        pub status: String,
        pub input_tokens: Option<i64>,
        pub output_tokens: Option<i64>,
        pub request: Json,
        pub response: Option<Json>,
        #[sea_orm(column_type = "Text")]
        pub error: Option<String>,
    }

    impl ActiveModelBehavior for ActiveModel {}

    #[derive(Clone, Copy, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
}

mod session_entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "agent_sessions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub workspace: String,
        pub api: String,
        pub model: String,
        pub endpoint: String,
        pub snapshot: Json,
        pub head_event_id: Option<String>,
        pub revision: i64,
        pub updated_at_ms: i64,
        pub uncertain_tools: bool,
    }

    impl ActiveModelBehavior for ActiveModel {}

    #[derive(Clone, Copy, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
}

mod session_event_entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "agent_session_events")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub parent_id: Option<String>,
        pub session_id: String,
        pub kind: String,
        pub payload: Json,
    }

    impl ActiveModelBehavior for ActiveModel {}

    #[derive(Clone, Copy, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
}

struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(CreateTrace), Box::new(CreateSessions)]
    }
}

#[derive(DeriveMigrationName)]
struct CreateTrace;

struct CreateSessions;

impl MigrationName for CreateSessions {
    fn name(&self) -> &str {
        "m20260927_000001_create_sessions"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateSessions {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("agent_sessions"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("workspace")).text().not_null())
                    .col(ColumnDef::new(Alias::new("api")).string().not_null())
                    .col(ColumnDef::new(Alias::new("model")).text().not_null())
                    .col(ColumnDef::new(Alias::new("endpoint")).text().not_null())
                    .col(ColumnDef::new(Alias::new("snapshot")).json().not_null())
                    .col(ColumnDef::new(Alias::new("head_event_id")).string())
                    .col(
                        ColumnDef::new(Alias::new("revision"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at_ms"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("uncertain_tools"))
                            .boolean()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("agent_session_events"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("parent_id")).string())
                    .col(ColumnDef::new(Alias::new("session_id")).string().not_null())
                    .col(ColumnDef::new(Alias::new("kind")).string().not_null())
                    .col(ColumnDef::new(Alias::new("payload")).json().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_agent_sessions_workspace_updated")
                    .table(Alias::new("agent_sessions"))
                    .col(Alias::new("workspace"))
                    .col(Alias::new("updated_at_ms"))
                    .col(Alias::new("id"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_agent_session_events_session")
                    .table(Alias::new("agent_session_events"))
                    .col(Alias::new("session_id"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("agent_session_events"))
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Alias::new("agent_sessions")).to_owned())
            .await
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateTrace {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("llm_traces"))
                    .col(
                        ColumnDef::new(Alias::new("request_id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("session_id")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("agent_run_id"))
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("provider_response_id")).string())
                    .col(ColumnDef::new(Alias::new("api")).string().not_null())
                    .col(ColumnDef::new(Alias::new("model")).text().not_null())
                    .col(
                        ColumnDef::new(Alias::new("started_at_ms"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("duration_ms"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("attempts")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("status")).string().not_null())
                    .col(ColumnDef::new(Alias::new("input_tokens")).big_integer())
                    .col(ColumnDef::new(Alias::new("output_tokens")).big_integer())
                    .col(ColumnDef::new(Alias::new("request")).json().not_null())
                    .col(ColumnDef::new(Alias::new("response")).json())
                    .col(ColumnDef::new(Alias::new("error")).text())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_llm_traces_session_cursor")
                    .table(Alias::new("llm_traces"))
                    .col(Alias::new("session_id"))
                    .col(Alias::new("started_at_ms"))
                    .col(Alias::new("request_id"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("llm_traces")).to_owned())
            .await
    }
}

#[derive(Clone)]
pub(crate) struct SqlStore {
    db: DatabaseConnection,
}

impl SqlStore {
    pub(super) async fn connect(url: &str) -> Result<Self, TraceError> {
        if url.starts_with("sqlite:") && !url.starts_with("sqlite::memory:") {
            let options = SqliteConnectOptions::from_str(url)
                .map_err(|_| TraceError("SQLite URL 无效".into()))?;
            let filename = options.get_filename();
            if let Some(parent) = filename.parent()
                && !parent.as_os_str().is_empty()
                && !url.contains("mode=memory")
            {
                std::fs::create_dir_all(parent)
                    .map_err(|_| TraceError("SQLite 目录创建失败".into()))?;
            }
        }
        let mut options = ConnectOptions::new(url);
        options
            .connect_timeout(Duration::from_secs(5))
            .acquire_timeout(Duration::from_secs(5));
        let db = Database::connect(options)
            .await
            .map_err(|_| TraceError("SQL 连接失败".into()))?;
        Migrator::up(&db, None)
            .await
            .map_err(|_| TraceError("SQL 迁移失败".into()))?;
        Ok(Self { db })
    }

    pub(super) async fn insert_one(&self, record: &TraceRecord) -> Result<(), TraceError> {
        let active: entity::ActiveModel = to_model(record).into();
        active
            .insert(&self.db)
            .await
            .map(|_| ())
            .map_err(|_| TraceError("SQL 写入失败".into()))
    }

    pub(super) async fn insert_many(&self, records: &[TraceRecord]) -> Result<(), TraceError> {
        if records.is_empty() {
            return Ok(());
        }
        entity::Entity::insert_many(
            records
                .iter()
                .map(|record| entity::ActiveModel::from(to_model(record))),
        )
        .exec(&self.db)
        .await
        .map(|_| ())
        .map_err(|_| TraceError("SQL 批量写入失败".into()))
    }

    pub(super) async fn get_one(
        &self,
        request_id: &str,
    ) -> Result<Option<TraceRecord>, TraceError> {
        entity::Entity::find_by_id(request_id.to_owned())
            .one(&self.db)
            .await
            .map_err(|_| TraceError("SQL 读取失败".into()))?
            .map(from_model)
            .transpose()
    }

    pub(super) async fn get_batch(
        &self,
        request_ids: &[String],
    ) -> Result<Vec<Option<TraceRecord>>, TraceError> {
        if request_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = entity::Entity::find()
            .filter(entity::Column::RequestId.is_in(request_ids.iter().cloned()))
            .all(&self.db)
            .await
            .map_err(|_| TraceError("SQL 批量读取失败".into()))?;
        let by_id: std::collections::HashMap<_, _> = rows
            .into_iter()
            .map(|row| (row.request_id.clone(), row))
            .collect();
        request_ids
            .iter()
            .map(|id| by_id.get(id).cloned().map(from_model).transpose())
            .collect()
    }

    pub(super) async fn list_session_page(
        &self,
        session_id: &str,
        after: Option<&TraceCursor>,
        limit: u64,
    ) -> Result<TracePage, TraceError> {
        let page_size = Ord::min(limit, 100);
        let mut query = entity::Entity::find().filter(entity::Column::SessionId.eq(session_id));
        if let Some(cursor) = after {
            query = query.filter(
                Condition::any()
                    .add(entity::Column::StartedAtMs.gt(cursor.started_at_ms))
                    .add(
                        Condition::all()
                            .add(entity::Column::StartedAtMs.eq(cursor.started_at_ms))
                            .add(entity::Column::RequestId.gt(cursor.request_id.clone())),
                    ),
            );
        }
        let rows = query
            .order_by_asc(entity::Column::StartedAtMs)
            .order_by_asc(entity::Column::RequestId)
            .limit(page_size + 1)
            .all(&self.db)
            .await
            .map_err(|_| TraceError("SQL 分页读取失败".into()))?;
        let has_more = rows.len() as u64 > page_size;
        let items: Vec<_> = rows
            .into_iter()
            .take(page_size as usize)
            .map(from_model)
            .collect::<Result<_, _>>()?;
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

impl SqlStore {
    pub(super) async fn save_session(
        &self,
        record: &SessionRecord,
        events: &[SessionEvent],
        expected_revision: Option<i64>,
    ) -> Result<(), TraceError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|_| TraceError("SQL 会话保存事务启动失败".into()))?;
        for event in events {
            let row = session_event_entity::Model {
                id: event.id.clone(),
                parent_id: event.parent_id.clone(),
                session_id: event.session_id.clone(),
                kind: event.kind.clone(),
                payload: event.payload.clone(),
            };
            // 冲突不触发 SQL 错误，避免 PostgreSQL 将事务置为失败；再核对内容保证重试安全。
            session_event_entity::Entity::insert(session_event_entity::ActiveModel::from(
                row.clone(),
            ))
            .on_conflict_do_nothing()
            .exec_without_returning(&transaction)
            .await
            .map_err(|_| TraceError("SQL 会话事件写入失败".into()))?;
            let stored = session_event_entity::Entity::find_by_id(&event.id)
                .one(&transaction)
                .await
                .map_err(|_| TraceError("SQL 会话事件读取失败".into()))?;
            if stored != Some(row) {
                return Err(TraceError("SQL 会话事件写入冲突".into()));
            }
        }
        let row = session_entity::Model {
            id: record.id.clone(),
            workspace: record.workspace.clone(),
            api: record.api.clone(),
            model: record.model.clone(),
            endpoint: record.endpoint.clone(),
            snapshot: record.snapshot.clone(),
            head_event_id: record.head_event_id.clone(),
            revision: record.revision,
            updated_at_ms: record.updated_at_ms,
            uncertain_tools: record.uncertain_tools,
        };
        if let Some(expected) = expected_revision {
            if record.revision != expected + 1 {
                return Err(TraceError("会话 revision 无效".into()));
            }
            let updated = session_entity::Entity::update_many()
                .set(session_entity::ActiveModel {
                    snapshot: ActiveValue::Set(row.snapshot),
                    head_event_id: ActiveValue::Set(row.head_event_id),
                    revision: ActiveValue::Set(row.revision),
                    updated_at_ms: ActiveValue::Set(row.updated_at_ms),
                    uncertain_tools: ActiveValue::Set(row.uncertain_tools),
                    ..Default::default()
                })
                .filter(session_entity::Column::Id.eq(&record.id))
                .filter(session_entity::Column::Revision.eq(expected))
                .exec(&transaction)
                .await
                .map_err(|_| TraceError("SQL 会话检查点更新失败".into()))?;
            if updated.rows_affected != 1 {
                return Err(TraceError(super::SESSION_REVISION_CONFLICT.into()));
            }
        } else {
            if record.revision != 0 {
                return Err(TraceError("初始会话 revision 无效".into()));
            }
            session_entity::ActiveModel::from(row)
                .insert(&transaction)
                .await
                .map_err(|_| TraceError("SQL 会话检查点创建失败或 ID 冲突".into()))?;
        }
        transaction
            .commit()
            .await
            .map_err(|_| TraceError("SQL 会话保存提交失败，重试可确认状态".into()))?;
        Ok(())
    }

    pub(super) async fn delete_session(&self, id: &str) -> Result<StoreDeletion, TraceError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|_| TraceError("SQL 会话删除事务启动失败".into()))?;
        let deleted = session_entity::Entity::delete_many()
            .filter(session_entity::Column::Id.eq(id))
            .exec(&transaction)
            .await
            .map_err(|_| TraceError("SQL 会话删除失败".into()))?;
        session_event_entity::Entity::delete_many()
            .filter(session_event_entity::Column::SessionId.eq(id))
            .exec(&transaction)
            .await
            .map_err(|_| TraceError("SQL 会话消息删除失败".into()))?;
        transaction
            .commit()
            .await
            .map_err(|_| TraceError("SQL 会话删除提交失败，重试可确认状态".into()))?;
        Ok(StoreDeletion {
            existed: deleted.rows_affected > 0,
            cleanup_error: None,
        })
    }

    pub(super) async fn load_session(&self, id: &str) -> Result<Option<SessionRecord>, TraceError> {
        session_entity::Entity::find_by_id(id.to_owned())
            .one(&self.db)
            .await
            .map(|row| row.map(from_session_model))
            .map_err(|_| TraceError("SQL 会话读取失败".into()))
    }

    pub(super) async fn load_session_event(
        &self,
        id: &str,
    ) -> Result<Option<SessionEvent>, TraceError> {
        session_event_entity::Entity::find_by_id(id.to_owned())
            .one(&self.db)
            .await
            .map(|row| {
                row.map(|row| SessionEvent {
                    id: row.id,
                    parent_id: row.parent_id,
                    session_id: row.session_id,
                    kind: row.kind,
                    payload: row.payload,
                })
            })
            .map_err(|_| TraceError("SQL 会话事件读取失败".into()))
    }

    pub(super) async fn list_sessions(
        &self,
        workspace: &str,
    ) -> Result<Vec<SessionRecord>, TraceError> {
        session_entity::Entity::find()
            .filter(session_entity::Column::Workspace.eq(workspace))
            .order_by_desc(session_entity::Column::UpdatedAtMs)
            .order_by_desc(session_entity::Column::Id)
            .limit(20)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(from_session_model).collect())
            .map_err(|_| TraceError("SQL 会话列表读取失败".into()))
    }

    pub(super) async fn list_all_sessions(&self) -> Result<Vec<SessionRecord>, TraceError> {
        session_entity::Entity::find()
            .order_by_desc(session_entity::Column::UpdatedAtMs)
            .order_by_desc(session_entity::Column::Id)
            .limit(20)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(from_session_model).collect())
            .map_err(|_| TraceError("SQL 会话列表读取失败".into()))
    }
}

fn from_session_model(row: session_entity::Model) -> SessionRecord {
    SessionRecord {
        id: row.id,
        workspace: row.workspace,
        api: row.api,
        model: row.model,
        endpoint: row.endpoint,
        snapshot: row.snapshot,
        head_event_id: row.head_event_id,
        revision: row.revision,
        updated_at_ms: row.updated_at_ms,
        uncertain_tools: row.uncertain_tools,
    }
}

fn to_model(record: &TraceRecord) -> entity::Model {
    entity::Model {
        request_id: record.request_id.clone(),
        session_id: record.session_id.clone(),
        agent_run_id: record.agent_run_id.clone(),
        provider_response_id: record.provider_response_id.clone(),
        api: record.api.clone(),
        model: record.model.clone(),
        started_at_ms: record.started_at_ms,
        duration_ms: record.duration_ms,
        attempts: record.attempts,
        status: match record.status {
            TraceStatus::Completed => "completed",
            TraceStatus::Failed => "failed",
            TraceStatus::TimedOut => "timed_out",
        }
        .into(),
        input_tokens: record.input_tokens,
        output_tokens: record.output_tokens,
        request: record.request.clone(),
        response: record.response.clone(),
        error: record.error.clone(),
    }
}

fn from_model(row: entity::Model) -> Result<TraceRecord, TraceError> {
    let status = match row.status.as_str() {
        "completed" => TraceStatus::Completed,
        "failed" => TraceStatus::Failed,
        "timed_out" => TraceStatus::TimedOut,
        _ => return Err(TraceError("SQL 记录状态无效".into())),
    };
    Ok(TraceRecord {
        request_id: row.request_id,
        session_id: row.session_id,
        agent_run_id: row.agent_run_id,
        provider_response_id: row.provider_response_id,
        api: row.api,
        model: row.model,
        started_at_ms: row.started_at_ms,
        duration_ms: row.duration_ms,
        attempts: row.attempts,
        status,
        input_tokens: row.input_tokens,
        output_tokens: row.output_tokens,
        request: row.request,
        response: row.response,
        error: row.error,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use super::{SessionEvent, SessionRecord, SqlStore};

    async fn fixture() -> (SqlStore, SessionRecord, SessionEvent, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("geer-checkpoint-{}", Uuid::new_v4()));
        let store = SqlStore::connect(&format!(
            "sqlite://{}?mode=rwc",
            root.join("sessions.db").display()
        ))
        .await
        .unwrap();
        let event = SessionEvent {
            id: Uuid::new_v4().to_string(),
            parent_id: None,
            session_id: Uuid::new_v4().to_string(),
            kind: "user".to_owned(),
            payload: json!({"text":"original"}),
        };
        let record = SessionRecord {
            id: event.session_id.clone(),
            workspace: root.to_string_lossy().into_owned(),
            api: "responses".to_owned(),
            model: "test".to_owned(),
            endpoint: "http://example.test/v1".to_owned(),
            snapshot: json!({"version":1}),
            head_event_id: Some(event.id.clone()),
            revision: 0,
            updated_at_ms: 1,
            uncertain_tools: false,
        };
        store
            .save_session(&record, std::slice::from_ref(&event), None)
            .await
            .unwrap();
        (store, record, event, root)
    }

    #[tokio::test]
    async fn failed_checkpoint_rolls_back_events_and_allows_identical_retry() {
        let (store, mut published, original, root) = fixture().await;
        published.revision = 1;
        store.save_session(&published, &[], Some(0)).await.unwrap();

        for (revision, expected) in [(1, Some(0)), (2, Some(0)), (0, None)] {
            let mut event = original.clone();
            event.id = Uuid::new_v4().to_string();
            event.parent_id = published.head_event_id.clone();
            let mut failed = published.clone();
            failed.revision = revision;
            failed.head_event_id = Some(event.id.clone());
            assert!(
                store
                    .save_session(&failed, std::slice::from_ref(&event), expected)
                    .await
                    .is_err()
            );
            assert!(store.load_session_event(&event.id).await.unwrap().is_none());
            assert_eq!(
                store.load_session(&published.id).await.unwrap(),
                Some(published.clone())
            );
        }

        published.revision = 2;
        store
            .save_session(&published, std::slice::from_ref(&original), Some(1))
            .await
            .unwrap();
        assert_eq!(
            store.load_session_event(&original.id).await.unwrap(),
            Some(original)
        );
        assert_eq!(
            store.load_session(&published.id).await.unwrap(),
            Some(published)
        );
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn conflicting_event_rolls_back_earlier_inserts() {
        let (store, published, original, root) = fixture().await;
        let mut new_event = original.clone();
        new_event.id = Uuid::new_v4().to_string();
        new_event.parent_id = Some(original.id.clone());
        let mut conflicting = original.clone();
        conflicting.payload = json!({"text":"conflicting"});
        let mut failed = published.clone();
        failed.revision = 1;
        failed.head_event_id = Some(new_event.id.clone());
        assert!(
            store
                .save_session(&failed, &[new_event.clone(), conflicting], Some(0))
                .await
                .is_err()
        );
        assert!(
            store
                .load_session_event(&new_event.id)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store.load_session_event(&original.id).await.unwrap(),
            Some(original)
        );
        assert_eq!(
            store.load_session(&published.id).await.unwrap(),
            Some(published)
        );
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
}
