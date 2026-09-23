# Design

## Context

Day1 REPL 已有 `config`、`provider/openai/chat` 和 `repl` 模块；当前 Chat Completions 实现持有会话历史、处理流式文本与 90 秒无正文超时。行为见本 change 的 spec delta，Responses 类型与事件参照 `docs/design/responses-api.md`。Day1 主 spec 尚未归档，本 change 以 Day1 delta 中的同名 requirement 为修改基线。

## Goals / Non-Goals

**Goals:** REPL 只面对小型 provider 接口；OpenAI provider 默认选 Responses，同时保留当前 Chat Completions 路径作为显式配置。两条路径一致地处理流式输出、完成标记、错误、会话重置。

**Non-Goals:** 不加入第二家服务商、自动回退、工具循环、全局 provider 注册表或运行时切换命令。

## Decisions

### Provider 边界与 API 选择

- `provider` 声明仅含 `stream_reply` 和 `reset` 的 crate-private trait；`repl` 通过该接口调用，不持有 SDK 类型。当前唯一实现是 `openai::Provider`，内部枚举分派 `Responses` 或现有 `Chat`。这样将教程的「模型调用」映射为 Rust 的静态类型边界，避免提前引入对象注册表或 `dyn` 生命周期。
- `config` 增加可选 `OPENAI_API`，接受 `responses`（默认）和 `chat-completions`。无效值在启动前报错。保留 `OPENAI_API_KEY`、`OPENAI_MODEL`、`OPENAI_BASE_URL` 的读取与优先级；不自动探测 endpoint，避免一次失败请求悄悄改变行为。

### Responses 会话与流

- 在现有 `async-openai` 0.42 依赖启用 `responses` feature；SDK 已有客户端、请求类型和 SSE 解析，`std` 不提供这些能力。继续用现有 `tokio`、`futures-util` 和 `dotenvy`，不加 crate。
- Responses 实现维护 `Vec<InputItem>`。发请求时克隆已完成历史，临时追加本轮 user 消息；仅在 `ResponseCompleted` 且有文本时，提交 user 消息及完成响应的 `output` 转换结果。保留完整输出项，包括 reasoning，为后续工具调用留下正确上下文。失败时不修改历史。相较 `previous_response_id`，手动上下文可保留本地 `/reset` 语义，也不要求兼容网关保存 response。
- 只将 `ResponseOutputTextDelta` 的正文传给终端；忽略控制和 reasoning 事件。`ResponseCompleted` 是成功边界，`ResponseFailed`、`ResponseIncomplete`、提前 EOF 和空正文是错误。请求建立与无可显示文本等待沿用 90 秒限期；Rust `Result` 将 SDK、I/O 与协议错误回传到 REPL。
- 现有 Chat Completions 实现保持原来的历史提交与回滚规则。统一 trait 方法使用同步 delta 回调，不改变终端输出与异步请求之间的所有权边界。

## Risks / Trade-offs

- [兼容网关只支持 Chat Completions] → 显式设置 `OPENAI_API=chat-completions`；默认请求失败时显示错误。
- [Responses 事件只有控制信息而没有正文] → 保留无正文超时；完成却无文本时明确失败。
- [历史随轮次增大] → 沿用 Day1 内存历史；压缩属于后续能力。

## Migration Plan

现有配置默认请求端点改变。只支持旧接口的用户在 `.env` 中加入 `OPENAI_API=chat-completions`；已支持 Responses 的配置不需修改。回滚可显式选择旧接口。
