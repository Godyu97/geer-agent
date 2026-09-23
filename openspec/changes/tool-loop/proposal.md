# Proposal

## Why

现有 REPL 只能流式聊天，无法让模型取得实时信息或调用本地能力。Day 2 先用时间工具打通调用、执行、回传、继续回答的循环。

## What Changes

- 模型可以请求 `get_current_time`，REPL 执行后回传真实本地时间，模型据此继续回答。
- 同一轮可顺序处理多个工具请求；调用过多、参数错误或未知工具时明确反馈。
- Responses 与 Chat Completions 两种已支持接口都可完成工具调用；关闭工具后维持普通聊天。

## Capabilities

### New Capabilities

- `tool-loop`：模型工具调用、结果回传及循环终止行为。

### Modified Capabilities

无。

## Non-goals

- 本变更不执行命令或读写文件。
- 不增加工具平台、权限规则或会话持久化。

## Impact

扩展现有 provider 和 REPL，新增最小工具定义及时间格式化依赖。保持 `cargo run` 为入口。
