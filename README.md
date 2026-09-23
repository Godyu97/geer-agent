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

需要将配置随单个二进制携带时，在项目根目录准备 `.env`，然后运行：

```sh
cargo build --release --features embed-env
```

构建产物位于 `target/release/geer-agent`，可复制到没有 `.env` 的目录运行。启用此 feature 时，构建目录缺少 `.env` 会导致编译失败。配置优先级为进程环境变量 > 运行目录 `.env` > 构建时内嵌的 `.env`。默认构建不包含 `.env`。

**内嵌的 `.env` 原文可从二进制提取，其中的 API key 不是加密存储。请只向可信对象分发此产物；修改内嵌配置后需重新构建。**
