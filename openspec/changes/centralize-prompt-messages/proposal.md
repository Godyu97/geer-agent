# Proposal

## Why

当前 `provider` 的 Chat Completions 和 Responses 分支分别保存、拼接对话消息，也分别决定工具结果和失败轮次怎样进入历史。`prompt` 只生成系统提示，使后续增加上下文或历史管理时必须同时改动两个协议分支。现在把消息所有权收敛到一处，保持现有交互行为。

## What Changes

- 两种 OpenAI 兼容接口继续收到相同顺序的系统上下文、用户输入、助手回复及工具调用结果。
- `/reset` 仍清空对话历史；模型请求失败时仍按是否已经执行工具保留或撤销本轮消息。
- 流式显示、重试、工具轮次上限、配置和错误提示保持现有行为。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

无。此变更仅调整内部消息管理，使用 `skip_specs: true`。

## Non-goals

- 不增加历史压缩、持久化、可编辑提示词或新的配置项。
- 不改变工具执行及授权规则，不新增模型协议或依赖。
- 不调整终端命令或可见文案。

## Impact

涉及 `src/prompt/`、`src/repl/`、`src/agent/`、`src/provider/` 之间的消息传递，以及对应的单元和集成测试。`prompt` 统一管理系统上下文、对话历史和本轮消息；`repl` 加载本轮用户消息；`provider` 只把已准备好的消息发送到对应接口并解析响应。
