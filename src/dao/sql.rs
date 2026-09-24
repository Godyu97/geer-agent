use std::time::Duration;

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, Database, DatabaseConnection, DbErr,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, sea_query::Condition,
};
use sea_orm_migration::prelude::*;

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

struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(CreateTrace)]
    }
}

#[derive(DeriveMigrationName)]
struct CreateTrace;

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

pub(crate) struct SqlStore {
    db: DatabaseConnection,
}

impl SqlStore {
    pub(super) async fn connect(url: &str) -> Result<Self, TraceError> {
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
