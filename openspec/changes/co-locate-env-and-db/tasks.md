# Tasks

## 1. 配置与路径

- [x] 1.1 在配置模块按可执行文件、普通 Cargo 构建时的项目目录、用户目录的顺序选取并加载 `.env`，内嵌构建跳过项目目录；用 `cargo test --bin geer-agent config::tests` 和实际 `cargo run` 验证路径选择。
- [x] 1.2 将默认 SQLite 地址改为选定目录的绝对 `.db/geer.sqlite`，保留显式数据库 URL；用 `cargo test --bin geer-agent config::tests` 验证默认和覆盖行为。

## 2. 运行验证与说明

- [x] 2.1 使用临时目录中的真实可执行文件验证程序旁 `.env`、用户目录回退、不同启动目录以及对应的 `.db/` 写入；运行 `cargo test --test config_paths` 和 `cargo test --test session_compaction`。
- [x] 2.2 更新 README 与 `.env.example` 的运行配置及数据迁移说明；运行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate co-locate-env-and-db --strict`。
