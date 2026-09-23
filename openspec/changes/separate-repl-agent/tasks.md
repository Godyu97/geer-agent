# Tasks

## 1. 协议与工具解耦

- [x] 1.1 在 `src/provider/mod.rs` 引入与协议无关的 `ToolSpec` / `ToolCall` / `ModelStep`，把 `ChatProvider` 从 `stream_reply(&mut Tools)` 改成单步方法（`begin_turn` / `complete_step` / `apply_tool_results` / `finish_turn` / `commit_turn` / `rollback_turn` / `reset`），并验证 `rg "tools::" src/provider` 无匹配
- [x] 1.2 把 `src/tools` 的 Chat / Responses 说明书改成 `specs()`，删除 `async-openai` 依赖与 `MAX_TOOL_ROUNDS`，并验证 `cargo test --lib tools::` 通过

## 2. 两种 API 只做单步

- [x] 2.1 把 `src/provider/openai/chat.rs` 的工具循环收成单步方法：空 id 在 `apply_tool_results` 补齐，`commit_turn` 为空操作，并验证 `cargo test --lib provider::openai::chat::` 通过
- [x] 2.2 把 `src/provider/openai/responses.rs` 的 `pending` / `last_output` 收进结构体，按现有提交语义实现单步方法，并验证 `cargo test --lib provider::openai::responses::` 通过

## 3. REPL 基底与 Agent 编排

- [x] 3.1 抽出 `repl::Session`（`handle_message` + `reset`），让 `repl::run` 只负责读入、命令、着色和循环，并验证 `cargo test --lib repl::` 通过且 `rg "tools::|provider::" src/repl` 无匹配
- [x] 3.2 新增 `src/agent/`：持有 `Provider` + `Tools`，实现工具循环与 `Session`，`main` 只调用 `agent::run()`；用假 `ChatProvider` 覆盖纯文本、一轮工具、五轮上限、失败 rollback/commit，并验证 `cargo test --lib agent::` 通过

## 4. 约定与回归

- [x] 4.1 在 `AGENTS.md` 的 Rust 约定中写明模块允许的引用方向，并验证该段与 `design.md` 模块图一致
- [x] 4.2 跑 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features`，确认集成测试仍覆盖工具对、失败保留历史和五轮上限
