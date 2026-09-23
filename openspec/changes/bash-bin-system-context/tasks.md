# Tasks

## 1. 配置与执行

- [x] 1.1 增加 Bash 可执行文件配置及启动版本探测，验证默认值、自定义路径和失败路径的单元测试。
- [x] 1.2 让 Bash 工具使用配置的可执行文件，验证工具执行测试覆盖自定义路径且 `cargo run` 能进入 REPL。

## 2. 环境提示

- [x] 2.1 在独立的 `prompt` 目录模块生成系统版本与 Bash 版本的默认系统提示，验证 Linux/Windows 格式及系统探测失败回退的测试。
- [x] 2.2 将同一提示传给 provider 并加入两种 API 的每次请求，验证模拟服务请求体及 `/reset` 后的行为。

## 3. 文档与检查

- [x] 3.1 更新 `.env.example` 与 README，检查配置名、默认值和错误行为一致。
- [x] 3.2 运行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate bash-bin-system-context --strict`。
