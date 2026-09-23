# Proposal

## Why

当前 Responses 请求或流在返回正文前失败时会直接结束本轮，短暂的网络或服务故障需要用户重新输入。用户希望在当前轮内自动重试，并看见重试进度。

## What Changes

- Responses 在尚未显示模型正文的请求失败后，最多重试五次，并在 Ai 输出区依次显示 `retry 1/5...` 到 `retry 5/5...`。
- 任一次成功后继续流式显示回答；全部重试失败时显示最终错误，保留此前完成的对话，并允许继续输入。
- 已显示部分正文后失败时直接报错，避免重试产生重复正文。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `repl-chat`：细化 Responses 请求失败后的重试、进度显示与最终错误行为。

## Non-goals

- 不改变 Chat Completions 的失败处理。
- 不加入 API 自动切换或用户可调的重试策略。
- 不重试终端输出失败。

## Impact

仅扩展现有 Responses 请求路径及相应测试；不增加依赖。对应 Day1 的 `repl-chat` 能力。
