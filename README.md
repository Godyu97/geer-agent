# geer-agent

GeekAgent 教程的 Rust 学习实现。

## 配置与运行

复制 `.env.example` 为 `.env`，填写 `OPENAI_API_KEY` 和 `OPENAI_MODEL`，然后运行：

```sh
cargo run
```

也可以只设置同名进程环境变量。`OPENAI_BASE_URL` 可选，默认使用 OpenAI 地址；`OPENAI_API` 可选，默认使用 Responses API，另可设为 `chat-completions`。

## 终端界面

在交互终端运行 `cargo run` 会进入全屏 TUI：左侧是可滚动的消息区，底部输入，宽度至少 60 列时右侧显示模型、会话、上下文估算与 token 用量。终端变窄时会隐藏面板。方向键编辑输入，PageUp/PageDown 或上下方向键滚动消息；`/help`、`/new`、`/sessions`、`/open`、`/compact` 等命令沿用原语义。工具首次授权会在界面内要求 `[y/N]`，只有输入 `y` 或 `yes` 才会同意。

面板的上下文 token 与占比是估算值；流式回答期间用 `~` 标记临时用量，完成后显示接口报告的真实 token 数。本轮统计包含该条输入触发的模型和自动压缩请求，累计统计涵盖本进程中的会话及手动压缩；若兼容接口未提供某次用量，面板显示“用量部分缺失”，估算值不会计作真实用量。`/exit`、Ctrl+C、Ctrl+D 会尝试保存会话并恢复原终端；全屏期间暂存的诊断记录在退出后写到 stderr。输入或输出接管道时继续使用原文本 REPL。

## 工具调用

REPL 向模型提供 `get_current_time`、`read`、`write`、`edit`、`bash`。模型选择工具后，程序执行并把结果交回模型继续回答。`GEER_AGENT_TOOLS=off` 可在启动时关闭所有工具；默认开启。服务端模型需要支持所选 API 的函数工具调用协议。

`read` 接受 `path`、可选的 1 起始行号 `offset` 和行数 `limit`，单次最多返回 2000 行、50 KiB 的 UTF-8 文本。`write` 接受 `path` 与 `content`，创建或覆盖文件。`edit` 接受 `path` 和 `edits` 数组，其中每项是 `oldText`、`newText`；旧文本必须在原文件中唯一匹配，各项不能重叠。`bash` 接受 `command`，在启动目录运行，10 秒超时，结果最多 2000 字符。

`GEER_AGENT_BASH_BIN` 可指定 Bash 可执行文件的绝对路径；未设置或为空时从 `PATH` 查找 `bash`。启动时会验证所选 Bash 并读取版本，路径或版本无效时直接报错退出。每次模型请求都会带默认系统提示，其中 `<context_data>` 包含系统版本和所选 Bash 版本；`/reset` 后仍会提供这些环境信息。

本机工具各自在当前会话首次使用时请求 `y/N` 授权。每次切换会话（包括切回旧会话）都会清空授权；打开失败或打开当前会话则保持授权。非交互输入无法确认时默认拒绝执行。

## 上下文压缩与会话恢复

`Prompt` 保留当前系统环境提示、历史摘要和近期原文。每次模型请求前估算上下文用量；默认窗口为 272,000 tokens，到 90% 时自动请求同一模型生成摘要。摘要保留任务目标、约束、进展和待办，并以普通历史背景交给模型；完整工具调用与结果不会在中间切开。估算依据消息的序列化体积和服务端报告的实际输入用量，可能与所用模型的 tokenizer 不同。

| 配置 | 默认 | 用途 |
| --- | --- | --- |
| `GEER_AGENT_CONTEXT_WINDOW_TOKENS` | `272000` | 当前模型的上下文窗口；请按实际模型调整 |
| `GEER_AGENT_AUTO_COMPACT` | `on` | 设为 `off` 时仅保留手动 `/compact`；明确的上下文溢出仍可压缩并重试一次 |
| `GEER_AGENT_SESSION_PERSISTENCE` | `on` | 设为 `off` 时只保留进程内多会话，不写会话检查点 |

`/new` 新建会话；`/open <session-id>` 打开进程内会话或当前目录存档；`/reset` 等同于 `/new`，`/resume <session-id>` 等同于 `/open`。`/sessions` 合并当前进程会话与最近 20 条存档，以 `*` 标记当前会话并显示保存状态。`/save` 重试所有待写会话。`/exit` 和 EOF 也会补写，并列出仍未保存的 UUID。启动时总是新建会话，历史会话需显式打开。`/help` 显示完整命令。

每条会话独立持有消息、摘要、上下文估算偏差及待写事件。数据库初始化失败时会告警并继续使用进程内多会话；写入失败时保留本地待写数据，切换会话也不会丢弃它。revision 冲突会显示在列表或保存结果中，程序不会自动覆盖数据库记录。跨进程只能恢复最后一次成功发布的检查点。同一工作目录中的文件仍由各会话共享。

会话存储将脱敏后的用户输入、模型输出、工具结果和摘要事件追加保存，并发布可恢复的检查点；压缩不会删除原始事件。恢复要求相同工作目录、模型、API 类型和端点，重新生成系统环境提示并清空工具授权。工具执行中断后会提示副作用未确认，不自动重跑工具。已知 API key、数据库 URL 和认证头在保存前脱敏；会话记录仍可能含其他敏感业务内容，请保护数据库及备份。本版不自动清理会话。

会话和 Trace 默认共用启动工作目录的 `./.db/geer.sqlite`，目录会自动创建且已加入 Git 忽略。可用公共配置改用其他数据库：

```sh
GEER_AGENT_DATABASE=sqlite
GEER_AGENT_DATABASE_URL='sqlite://.db/geer.sqlite?mode=rwc'
```

`GEER_AGENT_DATABASE` 默认为 `sqlite`；选择其他后端时必须设置对应 URL。Trace 和会话共用这组数据库配置，可分别通过 `GEER_AGENT_TRACE` 和 `GEER_AGENT_SESSION_PERSISTENCE` 关闭。已有数据库可以通过公共配置指定原地址，无需迁移数据。

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

启动时会显示 `Session ID`，输入 `/new` 或 `/reset` 后生成并显示新 UUID。每个模型步骤（包括摘要调用）有独立的 `request_id`，同一步骤的重试合并为一条逻辑记录。Trace 默认写入上述共用 SQLite；设 `GEER_AGENT_TRACE=off` 可关闭。

数据库后端支持 `sqlite`、`postgres`、`mysql`、`mongodb`，相应 URL 使用 `sqlite:`、`postgres://` 或 `postgresql://`、`mysql://`、`mongodb://` 或 `mongodb+srv://` 协议；MongoDB URI 必须带数据库名。配置可写在 `.env` 或进程环境变量中。SQL 数据库首次连接时自动运行版本化迁移；MongoDB 自动建立索引。Reader 提供代码接口，包括按 Request ID 读取和按 Session 游标分页，本次没有终端查询命令。

启用后，所选数据库会保存每次模型调用的**完整请求、合并后的响应或失败前已收到的内容**，其中可能包含对话历史、用户输入、工具参数与工具结果。已配置的 API key 与数据库连接串即使出现在内容中也会替换为 `[REDACTED]`。请保护数据库文件、服务与备份；本版不自动清理记录。记录不含 HTTP 认证头。数据库连接或写入失败会在终端报 Trace 告警，模型对话继续。

需要将配置随单个二进制携带时，在项目根目录准备 `.env`，然后运行：

```sh
cargo build --release --features embed-env
```

构建产物位于 `target/release/geer-agent`，可复制到没有 `.env` 的目录运行。启用此 feature 时，构建目录缺少 `.env` 会导致编译失败。配置优先级为进程环境变量 > 运行目录 `.env` > 构建时内嵌的 `.env`。默认构建不包含 `.env`。

**内嵌的 `.env` 原文可从二进制提取，其中的 API key 不是加密存储。请只向可信对象分发此产物；修改内嵌配置后需重新构建。**
