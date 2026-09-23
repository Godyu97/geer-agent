# Proposal

## Why

当前 REPL 直接使用 OpenAI Chat Completions 聊天类型。需要建立清晰的 provider 边界，让 OpenAI 默认使用 Responses API，同时允许只支持 Chat Completions 的兼容服务显式切换。

## What Changes

- **BREAKING**：不额外配置时，OpenAI 请求由 Chat Completions 切换为 Responses API；REPL 继续逐段显示回答并保留已完成轮次的上下文。
- 用户可显式选择 Chat Completions，以接入尚不支持 Responses 的 OpenAI 兼容服务。
- 请求失败仍显示错误并允许继续交互；`/reset` 清空所选接口的会话上下文。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `repl-chat`：模型请求改用 Responses API，同时保留流式多轮聊天行为。

## Non-goals

- 不在 Responses 请求失败时自动回退到 Chat Completions。
- 不改变 REPL 命令和现有对话交互方式。
- 不增加工具调用、持久化会话或其他模型服务商。

## Impact

- 在现有 OpenAI 模块内增加 Responses 实现和显式 API 选择，REPL 使用 provider 边界。
- SDK 启用 Responses 功能，不增加新的 crate；默认配置的自定义服务端需要提供 Responses API 兼容接口。
