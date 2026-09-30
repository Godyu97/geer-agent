use std::collections::HashSet;

use serde::Serialize;
use uuid::Uuid;

pub(crate) const DELETE_USAGE: &str = "用法：/delete [--yes] <完整 session-id> [session-id ...]";

pub(crate) fn delete_ids(ids: &[String]) -> Result<Vec<String>, String> {
    if ids.is_empty() {
        return Err(DELETE_USAGE.to_owned());
    }
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for id in ids {
        let parsed = Uuid::parse_str(id)
            .ok()
            .filter(|_| id.len() == 36)
            .ok_or_else(|| format!("删除需要完整 UUID：{id}\n{DELETE_USAGE}"))?;
        let id = parsed.to_string();
        if seen.insert(id.clone()) {
            unique.push(id);
        }
    }
    Ok(unique)
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct DeleteTarget {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) active: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct DeletePreview {
    pub(crate) targets: Vec<DeleteTarget>,
}

impl DeletePreview {
    pub(crate) fn ids(&self) -> Vec<String> {
        self.targets
            .iter()
            .map(|target| target.id.clone())
            .collect()
    }

    pub(crate) fn text(&self) -> String {
        let mut text = format!(
            "删除 {} 个会话？不可恢复；保留 Trace 日志。",
            self.targets.len()
        );
        for target in &self.targets {
            text.push_str(&format!(
                "\n{}  {}{}",
                target.id,
                target.title,
                if target.active { "（当前）" } else { "" }
            ));
        }
        if self.targets.iter().any(|target| target.active) {
            text.push_str("\n当前会话删除成功后，将在原 workspace 新建空会话。");
        }
        text
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeleteState {
    Deleted,
    Absent,
    Failed,
    CleanupPending,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct DeleteItem {
    pub(crate) id: String,
    pub(crate) state: DeleteState,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct DeleteReport {
    pub(crate) items: Vec<DeleteItem>,
    pub(crate) new_session_id: Option<String>,
}

impl DeleteReport {
    pub(crate) fn retry_ids(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|item| {
                matches!(
                    item.state,
                    DeleteState::Failed | DeleteState::CleanupPending
                )
            })
            .map(|item| item.id.clone())
            .collect()
    }

    pub(crate) fn text(&self) -> String {
        let mut lines = vec!["会话删除结果（Trace 日志保留）：".to_owned()];
        for item in &self.items {
            let label = match item.state {
                DeleteState::Deleted => "已删除",
                DeleteState::Absent => "已不存在",
                DeleteState::Failed => "删除失败",
                DeleteState::CleanupPending => "已删除，消息清理待重试",
            };
            lines.push(format!(
                "{} [{label}]{}",
                item.id,
                item.error
                    .as_ref()
                    .map_or(String::new(), |error| format!(" {error}"))
            ));
        }
        if let Some(id) = &self.new_session_id {
            lines.push(format!("已开始新会话。Session ID: {id}"));
        }
        lines.join("\n")
    }
}

pub(crate) struct StoreDeletion {
    pub(crate) existed: bool,
    pub(crate) cleanup_error: Option<String>,
}
