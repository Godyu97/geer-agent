# Tasks

## 1. Responses 重试

- [x] 1.1 在现有 Responses 请求路径加入最多五次有界重试与 Ai 进度输出，并阻止部分正文或输出错误后重试；用 `cargo check` 验证编译。

## 2. 验证与收尾

- [x] 2.1 用可控失败和成功的本地响应测试确认重试编号、六次尝试上限、成功提交历史和部分正文失败直接报错；运行相关 `cargo test`。
- [x] 2.2 执行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate retry-responses-request --strict`，确认变更可运行且规划有效。
