//! 无工具的交互基底：读入、命令、着色、循环。不引用 `tools` / `agent`。

mod color;
mod index;

pub(crate) use index::{Session, run};
