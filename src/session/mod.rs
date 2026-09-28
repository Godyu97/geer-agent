//! 会话持久化的数据契约；模型消息的所有权仍在 prompt。

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SessionEvent {
    pub(crate) id: String,
    pub(crate) parent_id: Option<String>,
    pub(crate) session_id: String,
    pub(crate) kind: String,
    pub(crate) payload: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SessionRecord {
    pub(crate) id: String,
    pub(crate) workspace: String,
    pub(crate) api: String,
    pub(crate) model: String,
    pub(crate) endpoint: String,
    pub(crate) snapshot: Value,
    pub(crate) head_event_id: Option<String>,
    pub(crate) revision: i64,
    pub(crate) updated_at_ms: i64,
    pub(crate) uncertain_tools: bool,
}
