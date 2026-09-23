# geer-agent

GeekAgent 教程的 Rust 学习实现。

## 配置与运行

复制 `.env.example` 为 `.env`，填写 `OPENAI_API_KEY` 和 `OPENAI_MODEL`，然后运行：

```sh
cargo run
```

也可以只设置同名进程环境变量。`OPENAI_BASE_URL` 可选，默认使用 OpenAI 地址；`OPENAI_API` 可选，默认使用 Responses API，另可设为 `chat-completions`。

需要将配置随单个二进制携带时，在项目根目录准备 `.env`，然后运行：

```sh
cargo build --release --features embed-env
```

构建产物位于 `target/release/geer-agent`，可复制到没有 `.env` 的目录运行。启用此 feature 时，构建目录缺少 `.env` 会导致编译失败。配置优先级为进程环境变量 > 运行目录 `.env` > 构建时内嵌的 `.env`。默认构建不包含 `.env`。

**内嵌的 `.env` 原文可从二进制提取，其中的 API key 不是加密存储。请只向可信对象分发此产物；修改内嵌配置后需重新构建。**
