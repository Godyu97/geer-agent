# Tasks

## 1. 读取

- [x] 1.1 注册 `read`、校验路径与行参数并实现有界 UTF-8 续读；用临时文件测试偏移、截断和无效文本。

## 2. 写入与编辑

- [x] 2.1 注册 `write` 并实现父目录创建及覆盖；用临时目录测试创建与覆盖。
- [x] 2.2 注册 `edit` 并实现原文件上唯一、非重叠的多处精确替换；用成功、缺失、重复、重叠和 CRLF 文件测试。

## 3. 授权与检查

- [x] 3.1 验证三种工具独立授权、绝对路径与 `/reset` 撤销；运行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate file-tools --strict`。
