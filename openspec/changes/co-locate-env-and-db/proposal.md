# Proposal

## Why

当前 `.env` 与默认 SQLite 文件都受启动工作目录影响。同一可执行文件从不同目录启动时，可能读到不同配置并写出分散的 `.db/`，不符合将程序与配置一起放置或统一放在用户目录的使用方式。

## What Changes

- **BREAKING**：运行时优先读取可执行文件同级的 `.env`；从本项目的 Cargo 构建目录运行且同级无文件时，使用项目根目录的 `.env`，以支持 `cargo run`；其余情况回退到 `~/.geer-agent/.env`。不再按启动工作目录或父目录寻找配置。
- 默认会话和 Trace 数据库写入最终选定配置目录下的 `.db/geer.sqlite`，因此 `.env` 与 `.db/` 位于同一目录；仅用进程环境或内嵌配置时，使用用户目录作为回退位置。
- 进程环境变量仍优先于选定的外部 `.env`，选定的外部 `.env` 仍优先于可选的内嵌配置；显式数据库 URL 仍按其自身地址使用。

## Capabilities

### New Capabilities

- `repl-chat`：补充运行时配置文件查找和默认本地数据库位置的行为契约（对应教程 Day1 的配置入口）。

### Modified Capabilities

无。

## Impact

涉及配置加载、默认 SQLite 地址、相关集成测试、README 和 `.env.example`。无需新依赖或自动搬迁已有数据库；原先工作目录中的默认数据库可通过显式 URL 继续使用。

## Non-goals

- 不改变显式指定的 SQLite 或远程数据库地址。
- 不迁移或删除已有 `.db/` 数据。
