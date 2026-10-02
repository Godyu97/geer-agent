use std::{cell::RefCell, error::Error, io};

use super::{Session, SessionScope, SessionStatus, Usage};
use crate::memory::{MemoryAction, MemoryEntry, MemoryPreview, MemoryService};
use crate::session::{
    DeleteItem, DeletePreview, DeleteReport, DeleteState, DeleteTarget, SessionEntry,
};

#[derive(Default)]
pub(crate) struct MockSession {
    pub(crate) seen: Vec<String>,
    pub(crate) fail: bool,
    pub(crate) actions: Vec<String>,
    pub(crate) scopes: RefCell<Vec<SessionScope>>,
    pub(crate) fail_open: bool,
    pub(crate) id: String,
    pub(crate) workspace: String,
    pub(crate) memory: MemoryService,
}

impl Session for MockSession {
    async fn memories(&self) -> Result<Vec<MemoryEntry>, Box<dyn Error>> {
        Ok(self.memory.list().await?)
    }
    async fn search_memory(&self, query: &str) -> Result<Vec<MemoryEntry>, Box<dyn Error>> {
        Ok(self.memory.search(query).await?)
    }
    async fn add_memory(&mut self, content: &str) -> Result<(MemoryEntry, bool), Box<dyn Error>> {
        Ok(self.memory.write(content).await?)
    }
    async fn edit_memory(
        &mut self,
        id: &str,
        content: &str,
    ) -> Result<(MemoryEntry, bool), Box<dyn Error>> {
        Ok(self.memory.edit(id, content).await?)
    }
    async fn preview_memory(&self, action: MemoryAction) -> Result<MemoryPreview, Box<dyn Error>> {
        Ok(self.memory.preview(action).await?)
    }
    async fn delete_memory(&mut self, id: &str) -> Result<bool, Box<dyn Error>> {
        Ok(self.memory.delete(id).await?)
    }
    async fn clear_memories(&mut self) -> Result<u64, Box<dyn Error>> {
        Ok(self.memory.clear().await?)
    }
    fn session_id(&self) -> &str {
        if self.id.is_empty() {
            "mock-session"
        } else {
            &self.id
        }
    }

    fn workspace(&self) -> String {
        if self.workspace.is_empty() {
            "/tmp/mock-workspace".into()
        } else {
            self.workspace.clone()
        }
    }

    fn status(&self) -> SessionStatus {
        SessionStatus {
            model: "mock".into(),
            session_id: self.session_id().into(),
            session_title: "新会话".into(),
            workspace: self.workspace(),
            context_tokens: 0,
            context_window_tokens: 100,
            turn_tokens: 0,
            total_tokens: 0,
            usage_complete: true,
            instructions_loaded: false,
            memory: self.memory.status(),
        }
    }

    async fn handle_message<F, U>(
        &mut self,
        input: &str,
        mut on_delta: F,
        mut on_usage: U,
    ) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
        U: FnMut(Option<Usage>) -> io::Result<()>,
    {
        self.seen.push(input.to_owned());
        on_delta("你")?;
        on_usage(Some(Usage {
            input: 3,
            output: 4,
        }))?;
        on_delta("好")?;
        if self.fail {
            return Err(io::Error::other("mock failure").into());
        }
        Ok(())
    }

    async fn new_session(&mut self) -> String {
        self.actions.push("new".into());
        self.id = "new-session".into();
        self.id.clone()
    }

    async fn flush(&mut self) -> String {
        self.actions.push("save".into());
        "已保存".into()
    }

    async fn compact(&mut self) -> Result<String, Box<dyn Error>> {
        self.actions.push("compact".into());
        Ok("已压缩".into())
    }

    async fn set_workspace(&mut self, path: &str) -> Result<String, Box<dyn Error>> {
        self.actions.push(format!("workspace:{path}"));
        self.workspace = path.to_owned();
        Ok(format!("已切换 Workspace: {path}"))
    }

    async fn sessions(&self, scope: SessionScope) -> Result<Vec<String>, Box<dyn Error>> {
        self.scopes.borrow_mut().push(scope);
        Ok(vec!["mock-session".into()])
    }

    async fn session_entries(&self, _: SessionScope) -> Result<Vec<SessionEntry>, Box<dyn Error>> {
        Ok(Vec::new())
    }

    async fn open(&mut self, id: &str) -> Result<String, Box<dyn Error>> {
        if self.fail_open {
            return Err(io::Error::other("cannot open").into());
        }
        self.actions.push(format!("open:{id}"));
        self.id = id.to_owned();
        Ok(format!("已打开 {id}"))
    }

    async fn preview_delete(&self, ids: &[String]) -> Result<DeletePreview, Box<dyn Error>> {
        Ok(DeletePreview {
            targets: ids
                .iter()
                .map(|id| DeleteTarget {
                    id: id.clone(),
                    title: "测试会话".into(),
                    active: false,
                })
                .collect(),
        })
    }

    async fn delete_sessions(&mut self, ids: &[String]) -> Result<DeleteReport, Box<dyn Error>> {
        self.seen.extend(ids.iter().cloned());
        Ok(DeleteReport {
            items: ids
                .iter()
                .map(|id| DeleteItem {
                    id: id.clone(),
                    state: DeleteState::Deleted,
                    error: None,
                })
                .collect(),
            new_session_id: None,
        })
    }
}
