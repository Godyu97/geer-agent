# Tasks

- [x] 1. 在 Cargo 中增加可选 `embed-env` feature，并在配置加载中接入内嵌 `.env`；用 `cargo test` 和 `cargo test --features embed-env` 验证两种构建。
- [x] 2. 记录构建命令、覆盖顺序与内嵌密钥分发注意事项；把产物移到无 `.env` 的目录执行 `/exit`，验证单文件启动。
- [x] 3. 运行 `cargo fmt --all`、相关测试、`cargo clippy --all-targets --all-features` 与 `openspec validate embed-env-config --strict`。
