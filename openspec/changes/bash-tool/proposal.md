# Proposal

## Why

Day 2 已让模型调用安全的时间工具。Day 3 要让同一循环执行本机命令，以便模型检查项目和运行构建，同时让用户掌握命令授权。

## What Changes

- 模型可请求 `bash` 并取得命令输出及退出状态。
- 首次调用时向用户展示命令并询问授权；同一会话中已授权的 `bash` 可继续调用，拒绝则不执行。
- 命令执行有 10 秒超时，过长输出会截断；`/reset` 清除授权。

## Capabilities

### New Capabilities

- `bash-tool`：本机命令的授权、执行和结果限制。

### Modified Capabilities

无。

## Non-goals

- 不实现复杂权限规则、容器隔离或后台命令会话。
- 不让命令未经用户首次授权直接执行。

## Impact

在现有工具清单加入 Bash，并使 REPL 的输入读取可用于授权提示。运行入口仍是 `cargo run`。
