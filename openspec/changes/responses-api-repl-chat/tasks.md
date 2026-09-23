# Tasks

## 1. 配置与边界

- [x] 1.1 为 `async-openai` 启用 Responses feature，并增加 `OPENAI_API` 解析、默认值及无效值检查；用 `cargo test` 验证配置。
- [x] 1.2 建立最小 provider 接口与 OpenAI 分派，让 REPL 通过接口调用并保留 Chat Completions 实现；用 `cargo check` 验证两种配置可编译。

## 2. Responses 实现

- [x] 2.1 增加 Responses 流式请求及完成事件处理，保留已完成上下文与 `/reset` 语义；用可控事件流测试成功、失败、空回复、超时及历史提交。
- [x] 2.2 用本地模拟 Responses endpoint 验证默认请求路径、多轮上下文、失败后继续交互；用 `cargo run` 验证显式 Chat Completions 路径。

## 3. 收尾

- [x] 3.1 更新 `.env.example` 说明接口选择；执行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate responses-api-repl-chat --strict`。
