# Tasks

## 1. 工具基础

- [x] 1.1 增加工具开关、时间工具定义与参数校验；用 `cargo test` 验证关闭和无效参数路径。

## 2. 模型循环

- [x] 2.1 为 Chat Completions 收集分片调用、回传工具结果并继续请求；用可控流测试调用配对和失败历史。
- [x] 2.2 为 Responses 收集完成响应中的调用、回传工具结果并继续请求；用可控流测试多调用与五轮上限。

## 3. 交互验收

- [x] 3.1 显示简短工具进度并补充运行说明；运行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate tool-loop --strict`，再用 `cargo run` 询问当前时间。
