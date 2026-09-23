# Tasks

## 1. 配置与依赖

- [x] 1.1 添加 SDK、runtime、stream 和 dotenv 依赖，补充 `.env.example`、忽略本地 `.env`，实现必需配置检查与默认 endpoint；用配置单元测试和 `cargo test` 验证。

## 2. 流式聊天

- [x] 2.1 实现 OpenAI Chat Completions 流式请求与会话历史提交/回滚；用历史状态单元测试和 `cargo test` 验证成功提交与失败回滚。

## 3. REPL

- [x] 3.1 接入 `src/repl/index.rs` 的输入循环、命令分发和逐块终端输出；用命令解析单元测试覆盖 `/help`、`/reset`、`/exit`、空行与未知命令。
- [x] 3.2 手工运行 `cargo run` 验证连续对话、即时流式输出、reset 清空上下文、请求失败后仍可继续输入及 EOF 正常退出。

## 4. 收尾

- [x] 4.1 执行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate day1-repl-chat --strict`，修复发现的问题。

## 5. REPL 稳定性与显示

- [x] 5.1 按字节读取输入行，遇无效 UTF-8 提示重试且保留对话；用有效/无效字节的输入单元测试验证。
- [x] 5.2 限定请求及流式无正文等待时间，收到完成标记及时结束，空回复或提前断流明确报错并回滚；用可控流测试验证超时及空回复。
- [x] 5.3 实现 `src/repl/color.rs`，在交互终端为“李火旺🔥”和“Ai”及其回复使用区分颜色，非交互或 `NO_COLOR` 时输出纯文本；用 `cargo run` 手工检查提示与回复。
- [x] 5.4 执行 `cargo fmt --all`、相关 `cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate day1-repl-chat --strict`，修复发现的问题。
