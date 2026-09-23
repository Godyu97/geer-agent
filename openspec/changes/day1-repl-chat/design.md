# Design

## Context

入口 `src/main.rs` 当前只打印占位文本，Cargo 尚无依赖。变更采用仓库既定的 `repl-chat` 能力与教程 Day 1 范围；行为契约见 `specs/repl-chat/spec.md`。

## Goals / Non-Goals

**Goals:** 用少量 Rust 模块建立可运行的异步流式对话循环，并让本地开发者能用 `.env` 配置模型。

**Non-Goals:** 抽象多 provider、建立复杂 CLI/TUI、增加工具调用、历史持久化或并发会话。

## Decisions

### SDK 与异步运行时

- 使用社区 crate `async-openai` 0.42 的 Chat Completions 功能。它是检索中最成熟、采用度最高的 Rust OpenAI SDK；SDK 提供 OpenAI 类型、客户端和 SSE 流解析，也支持自定义 API base。它并非 OpenAI 官方维护的 Rust SDK。
- 使用 `tokio` 的单线程 runtime 执行 SDK 的异步 HTTP 请求。标准输入在每个请求前顺序读取；程序没有并发后台任务，因此不需要线程池或异步终端输入。
- 使用 `futures-util` 的 `StreamExt` 逐个读取 SDK 返回的流。标准库没有通用 `Stream` 扩展方法，且输出必须在完整响应到达前发生。
- 使用 `dotenvy` 加载可选 `.env`。标准环境变量优先于文件值；文件不存在时继续读取进程环境。`std` 本身不解析 dotenv 文件。

### 配置与错误

- `config` 模块读取 `OPENAI_API_KEY`、`OPENAI_MODEL`、`OPENAI_BASE_URL`，对 key/model 做空值检查；base URL 缺省时使用 `https://api.openai.com/v1`。
- `OPENAI_API_KEY` 与模型名均不设隐式默认值，避免程序在错误凭证或未知模型下静默发请求。`.env.example` 只放变量名和示例占位值，`.env` 加入 `.gitignore`。
- 配置错误在 REPL 启动前返回；OpenAI SDK 错误在当前轮次显示并恢复输入循环。使用标准 `Result`/错误类型，不引入额外错误处理 crate。

### 模块与会话历史

- 保留 `main.rs` 为薄启动入口；代码分别放在 `src/config/mod.rs`、`src/provider/openai/chat.rs`、`src/repl/index.rs`，用标准 `mod.rs` 接入模块，不建立 provider trait。
- `Chat` 持有 SDK client、model 和 `Vec<ChatCompletionRequestMessage>`，历史消息与 Chat Completions 请求格式一致。由于 SDK 请求取得消息向量所有权，每轮请求克隆当前历史；Day 1 历史量有限，这优先保证初学者可理解的所有权边界。
- 一轮开始时追加 user 消息；流式消费时把非空 delta 交给 REPL 输出，并在局部累积完整回答。流结束成功后追加 assistant 消息；请求或流失败则移除这条 user 消息，不保存部分回答。
- `Chat::stream_reply` 接收一个可返回输出错误的同步 delta 回调，避免额外 async-stream 依赖；REPL 在回调内打印并 flush。输入按顺序处理，流式回答期间不会开始下一轮或解析命令。
- `index` 使用标准输入行循环并分发 `/help`、`/reset`、`/exit`。空行跳过；未知斜杠命令提示后继续；EOF 视为正常退出。
- 输入改为按字节读取整行，再验证 UTF-8；无效字节只丢弃当前行并提示重试，避免 `read_line` 的解码错误结束整个 REPL。保持阻塞式单会话输入，不增加异步终端依赖。
- 请求建立和流式等待都设置 90 秒无可显示文本期限；收到文本后重置期限，心跳与空片段不重置。收到模型完成标记即结束读取，避免代理未发送最终关闭标记时一直等待；空回复或未完成即断流视为失败并回滚当前轮。`tokio` 只补 `time` feature，不增加 crate。
- `src/repl/color.rs` 用标准 ANSI 转义码包装用户标签、模型标签和回复片段；仅在 stdout 为交互终端、`TERM` 非 `dumb` 且未设置 `NO_COLOR` 时启用。每次写完片段立即复位颜色，避免等待输入或中断时遗留终端样式。用户标签用青色，模型回复用绿色。
- 回复片段写入和刷新 stdout 的错误向上传递到本轮失败路径，避免输出失败却把回答保存为成功历史。

### 验证

- 配置解析从环境变量读取与必需字段检查用单元测试覆盖；命令解析和空行处理用单元测试覆盖。
- 真实 API 的流式首片输出、多轮上下文、reset 和失败后恢复通过手工验收，避免测试依赖用户密钥或外部网络。

## Risks / Trade-offs

- `[外部 API 不可用或配置不正确]` → 在启动错误或请求错误中显示原因；请求失败后回滚当前 user 消息并保留 REPL。
- `[历史随对话增长而扩大]` → Day 1 有意保留完整内存历史；后续压缩或持久化属于各自能力 change。
- `[标准输入是阻塞读取]` → 仅在等待用户输入时阻塞，符合单会话、串行 REPL 的设计；暂不支持回答过程中打断或并行输入。
- `[模型思考时间超过 90 秒且没有正文]` → 本轮会超时并回滚，用户可以重试；固定期限优先避免终端无限等待。

## Migration Plan

无需迁移已有数据。用户复制 `.env.example` 为 `.env` 并填写 key 与 model 后运行 `cargo run`；没有 `.env` 时可直接导出同名环境变量。
