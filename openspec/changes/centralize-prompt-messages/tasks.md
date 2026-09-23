# Tasks

## 1. 建立统一消息状态

- [x] 1.1 在 `src/prompt/` 增加持有系统上下文、历史和本轮状态的 `Prompt`，提供按协议生成请求快照及 `begin_turn` / `reset`；用单元测试验证 system 上下文始终置前、跨轮历史顺序和 reset 后只清对话，并运行 `cargo test prompt::`。
- [x] 1.2 把工具结果、助手输出、成功提交与失败回滚的状态转移移到 `Prompt`；用单元测试分别验证 Chat 缺失 call id 的补齐、Responses 完整输出项保留、首次失败撤销与工具后失败保留，并运行 `cargo test prompt::`。

## 2. 协议层只处理请求与响应

- [x] 2.1 调整 Chat Completions 的 `complete_step` 接收 `Prompt` 消息快照，移除协议结构内的系统提示及历史写入，同时保留现有流式解析；运行 `cargo test provider::openai::chat::` 并检查发出的请求体顺序。
- [x] 2.2 调整 Responses 的 `complete_step` 接收 `Prompt` 消息快照，把完整模型输出交回 `Prompt`，移除协议结构内的 `history` / `pending` / `last_output`，保持重试与超时行为；运行 `cargo test provider::openai::responses::` 及 `cargo test --test responses_retry`。

## 3. REPL 加载消息并保持工具循环

- [x] 3.1 修改 `repl::run` 和 `Session`，让 REPL 在普通输入时调用 `Prompt::begin_turn`，在 `/reset` 时清空 `Prompt` 并继续清工具授权；运行 `cargo test repl::`，验证空输入和命令不会进入历史。
- [x] 3.2 调整 `agent` 从 `Prompt` 取得每一步消息、记录模型步骤与工具结果，并沿用现有五轮上限及提交/回滚分支；运行 `cargo test agent::`，验证纯文本、工具轮次、首次失败和工具后失败。

## 4. 回归验收

- [x] 4.1 用现有或补充的请求体集成测试覆盖两种接口的多轮上下文、工具调用后继续对话、失败后继续对话与 `/reset`；运行 `cargo test --test tool_loop` 和 `cargo test --test responses_retry`。
- [x] 4.2 运行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features`，并检查 `src/provider/` 不再持有或拼接系统提示、历史、本轮待提交消息；在已配置接口的本地 `cargo run` 中验证仍可对话和执行 `/reset`。
