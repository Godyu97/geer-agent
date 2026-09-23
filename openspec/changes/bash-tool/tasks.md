# Tasks

## 1. 授权入口

- [x] 1.1 让 REPL 在工具执行时读取首次授权，拒绝或无交互输入时默认不执行；用授权/拒绝测试验证，`cargo run` 可继续普通对话。

## 2. 命令工具

- [x] 2.1 注册 `bash` 并在授权后执行命令，返回输出和退出状态；用短命令及非零退出单元测试验证。
- [x] 2.2 加入 10 秒超时和 2000 字符结果限制；用超时与大量输出测试验证。

## 3. 检查

- [x] 3.1 验证 `/reset` 清除 Bash 授权；运行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate bash-tool --strict`。
