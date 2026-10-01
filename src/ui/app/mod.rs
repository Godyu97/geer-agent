//! 图形会话共用层；具体窗口、HTTP 与浏览器凭证留在宿主。

pub(super) mod authorization;
mod close;
mod commands;
#[cfg(any(feature = "gui", feature = "web"))]
mod events;
#[cfg(any(feature = "gui", feature = "web"))]
mod runtime;

#[cfg(any(feature = "gui", feature = "web"))]
pub(super) use events::AppEvent;
#[cfg(feature = "web")]
pub(super) use events::EventSink;
#[cfg(any(feature = "gui", feature = "web"))]
pub(super) use runtime::{AppRuntime, Host};
