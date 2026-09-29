# Proposal

## Why

当前交互界面只适合终端；用户需要在 Linux Wayland 与 Windows 11 使用窗口界面，同时保留现有 TUI、模型调用、会话和工具权限语义。先让同一程序能从 `.env` 选择 GUI，并将已有能力图形化。

## What Changes

- 构建包含 GUI 能力时，用户可用环境配置从统一 `geer-agent` 入口打开桌面窗口；默认交互终端及管道行为保持现状。未编入 GUI 时请求 GUI 会明确失败。
- 窗口提供流式聊天、Markdown、完整会话历史、会话列表及切换、用量状态和已有命令。
- 工具调用在窗口内按当前会话的授权规则确认；拒绝或关闭窗口不会默认执行待授权工具。
- 关闭窗口时补写待保存会话；保存失败可见并可重试。

## Capabilities

### New Capabilities

- `gui`: 桌面窗口选择、会话显示与交互、授权、跨平台输入和关闭保存行为。

### Modified Capabilities

无。现有会话原始事件和检查点语义不变，GUI 使用已有记录重建展示历史。

## Impact

`src/main.rs`、`src/ui/`、`src/interaction/`、`src/prompt/`、环境配置及构建配置增加 GUI 接入；GUI Rust 与前端代码位于 `src/ui/gui/`。新增可选 Tauri 与 React/Vite 构建依赖。默认 Cargo 构建仍为纯终端程序；数据库结构、模型协议与工具执行规则不变。本能力独立于教程 Day 7 的 `tui`，使用新能力 id `gui`。

## Non-goals

本轮不增加安装包、双击启动、目录选择器、配置编辑器、文件树、多窗口或停止生成。
