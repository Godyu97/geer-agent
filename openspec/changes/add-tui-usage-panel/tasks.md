# Tasks

## 1. 接入基础

- [x] 1.1 增加 Ratatui/Crossterm 依赖与 `src/ui/tui/` 模块，由 `src/ui/mod.rs` 组合 Agent 和默认 TUI、非 TTY 保留文本 REPL，`main.rs` 只启动；运行 `cargo build` 并用管道 `/exit` 验证旧入口。
- [x] 1.2 将会话状态和通用命令解析移到界面无关的 `src/interaction/`，Agent/TUI 互不引用；运行 `cargo test repl::` 并检查模块引用方向。

## 2. 交互与显示

- [x] 2.1 实现备用屏幕、宽窄布局、消息滚动、底部单行编辑与窗口调整；用 TUI 渲染/按键测试和真实终端 `/help`、`/exit` 验证。
- [x] 2.2 将流式回答、工具进度、现有会话命令及授权确认接入 TUI，暂存全屏期间 stderr 诊断；用界面内 `/new`、`/sessions`、拒绝授权的测试或可重复手工步骤验证。

## 3. 用量与收尾

- [x] 3.1 显示模型、会话、上下文估算、本轮与累计 token；流式估算和真实用量/缺失状态分别标示，压缩后刷新；运行相关单元测试并核对两种模型协议用量路径。
- [x] 3.2 补充 README 的界面使用说明；运行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features`、`openspec validate add-tui-usage-panel --strict`，并用真实终端检查退出恢复。
