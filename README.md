# geer-agent

GeekAgent 教程的 Rust 学习实现。

## 配置与运行

复制 `.env.example` 为 `.env`，填写 `OPENAI_API_KEY` 和 `OPENAI_MODEL`，然后运行：

```sh
cargo run
```

也可以只设置同名进程环境变量。`OPENAI_BASE_URL` 可选，默认使用 OpenAI 地址；`OPENAI_API` 可选，默认使用 Responses API，另可设为 `chat-completions`。

## 工具调用

REPL 向模型提供 `get_current_time`、`read`、`write`、`edit`、`bash`。模型选择工具后，程序执行并把结果交回模型继续回答。`GEER_AGENT_TOOLS=off` 可在启动时关闭所有工具；默认开启。服务端模型需要支持所选 API 的函数工具调用协议。

`read` 接受 `path`、可选的 1 起始行号 `offset` 和行数 `limit`，单次最多返回 2000 行、50 KiB 的 UTF-8 文本。`write` 接受 `path` 与 `content`，创建或覆盖文件。`edit` 接受 `path` 和 `edits` 数组，其中每项是 `oldText`、`newText`；旧文本必须在原文件中唯一匹配，各项不能重叠。`bash` 接受 `command`，在启动目录运行，10 秒超时，结果最多 2000 字符。

`GEER_AGENT_BASH_BIN` 可指定 Bash 可执行文件的绝对路径；未设置或为空时从 `PATH` 查找 `bash`。启动时会验证所选 Bash 并读取版本，路径或版本无效时直接报错退出。每次模型请求都会带默认系统提示，其中 `<context_data>` 包含系统版本和所选 Bash 版本；`/reset` 后仍会提供这些环境信息。

四个本机工具各自在本次 REPL 会话首次使用时请求 `y/N` 授权。授权覆盖该工具本会话内的后续调用以及任意本机路径；`/reset` 清除对话和授权。非交互输入无法确认时默认拒绝执行。

## 执行预算

每条输入默认在第 12 个模型响应后提示收敛；若近期持续取得新结果，通用提示可推迟到第 15 个响应。最多使用 30 个模型响应、100 次实际工具调用和 10 分钟。单次响应中的多个已授权 `read` 可以并行执行；`write`、`edit`、`bash` 按调用顺序执行。连续相同结果、连续错误或连续无进展会先提示模型改变方法，仍继续时关闭工具并请求最终回答。

以下进程环境变量或 `.env` 项可选：

| 名称 | 含义 |
| --- | --- |
| `GEER_AGENT_MAX_DURATION_SECONDS` | 单条输入的总时长上限，默认 600 |
| `GEER_AGENT_MAX_INPUT_TOKENS` / `GEER_AGENT_MAX_OUTPUT_TOKENS` | 模型报告的累计输入/输出 Token 上限 |
| `GEER_AGENT_MAX_COST_USD` | 按显式单价计算的美元费用上限 |
| `GEER_AGENT_INPUT_USD_PER_MILLION_TOKENS` / `GEER_AGENT_OUTPUT_USD_PER_MILLION_TOKENS` | 每百万输入/输出 Token 的美元单价；配置费用上限时两项都必填 |

启用 Token 或费用上限后，兼容接口若不返回 Token 用量，程序会停止继续执行工具并尝试最终回答。Chat Completions 会请求流式用量尾包；兼容接口可能不支持。每次运行及工具调用的结构化指标写到 stderr，含结束原因、用量、耗时和摘要，不含工具参数、结果正文或 API key。

## LLM 调用 Trace

启动时会显示 `Session ID`，输入 `/reset` 后生成并显示新 ID。每个模型步骤有独立的 `request_id`，同一步骤的重试合并为一条逻辑记录。默认不持久化；设置以下两项可选择一种数据库：

```sh
GEER_AGENT_TRACE_DATABASE=sqlite
GEER_AGENT_TRACE_DATABASE_URL='sqlite://trace.sqlite?mode=rwc'
```

`GEER_AGENT_TRACE_DATABASE` 支持 `sqlite`、`postgres`、`mysql`、`mongodb`，相应 URL 使用 `sqlite:`、`postgres://` 或 `postgresql://`、`mysql://`、`mongodb://` 或 `mongodb+srv://` 协议；MongoDB URI 必须带数据库名。可在 `.env` 或进程环境变量中配置，一次只选择一个数据库。SQL 数据库首次连接时自动运行版本化迁移；MongoDB 自动建立索引。Reader 提供代码接口，包括按 Request ID 读取和按 Session 游标分页，本次没有终端查询命令。

启用后，所选数据库会保存每次模型调用的**完整请求、合并后的响应或失败前已收到的内容**，其中可能包含对话历史、用户输入、工具参数与工具结果。已配置的 API key 与数据库连接串即使出现在内容中也会替换为 `[REDACTED]`。请保护数据库文件、服务与备份；本版不自动清理记录。记录不含 HTTP 认证头。数据库连接或写入失败会在终端报 Trace 告警，模型对话继续。

需要将配置随单个二进制携带时，在项目根目录准备 `.env`，然后运行：

```sh
cargo build --release --features embed-env
```

构建产物位于 `target/release/geer-agent`，可复制到没有 `.env` 的目录运行。启用此 feature 时，构建目录缺少 `.env` 会导致编译失败。配置优先级为进程环境变量 > 运行目录 `.env` > 构建时内嵌的 `.env`。默认构建不包含 `.env`。

**内嵌的 `.env` 原文可从二进制提取，其中的 API key 不是加密存储。请只向可信对象分发此产物；修改内嵌配置后需重新构建。**
