# Proposal

## Why

Agent 目前把启动目录同时固定为会话归属、工具执行目录和模型环境上下文，启动后无法安全切换项目。用户需要在不中断进程的情况下选择 workspace，并让会话、工具、提示词和三种界面始终使用同一目录。

## What Changes

- 支持查看和设置当前 workspace；成功切换时保存旧会话并在目标目录新建会话，无效目录不改变当前状态。
- 让每条会话独立保存 workspace；打开旧会话时恢复其目录，默认列表只看当前目录并可查看全部目录。
- 让 Bash、文件和查询工具以及模型环境提示统一使用活动会话的 workspace。
- 在文本 REPL、TUI 和桌面 GUI 中提供 workspace 设置与状态入口；GUI 支持系统目录选择。
- 切换或恢复会话时继续清空工具授权，保存失败保留旧会话的待补写内容。

## Capabilities

### New Capabilities

- `workspace-management`: 定义 workspace 的路径校验、切换语义、命令以及 REPL、TUI、GUI 的可见入口。

### Modified Capabilities

- `session-persistence`: 会话持久化和恢复从“必须处于相同启动目录”改为保存并恢复会话自己的 workspace，并支持当前目录与全部目录两种列表范围。
- `tool-loop`: 工具执行目录由活动会话的 workspace 决定，切换后相对路径和 Bash 工作目录同步更新。

## Impact

影响会话状态和存储协调、提示词环境组装、工具执行入口、公共交互命令，以及 REPL、TUI、Tauri/React GUI。数据库继续复用现有 `workspace` 字段，无数据迁移。GUI feature 增加 Tauri dialog 可选依赖及最小打开目录权限。

## Non-goals

- 不修改进程 cwd、配置文件选择或默认数据库位置。
- 不增加最近 workspace、收藏、会话移动、目录创建、沙箱或跨机器同步。
- 不改变现有逐工具、按会话授权规则，也不自动执行 Shell 路径展开。
