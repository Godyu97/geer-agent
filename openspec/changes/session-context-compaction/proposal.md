# Proposal

## Why

长对话和工具结果持续累加，最终会耗尽模型上下文，或使旧信息干扰当前任务。现有 Session ID 仅供 Trace 关联，退出后无法继续同一会话。用户要求结合 GeekAgent Day5 和主流 Agent 的上下文管理方法，在当前 Rust 架构上完成压缩与恢复。

## What Changes

- 上下文用量达到默认 272,000 tokens 窗口的 90% 时自动摘要旧历史，保留近期完整消息；新增 `/compact` 手动入口和可见进度。
- 一次工具循环中的工具结果也可触发压缩；失败时保留原上下文，不自动重复执行已完成工具。
- 可选择持久化会话原始记录和安全检查点；新增 `/sessions`、`/resume <session-id>`，退出后可以继续相同配置的会话。
- 数据库写入失败时告警并继续，恢复结果以最后成功保存的检查点为准；恢复不自动重跑工具，工具授权重新确认。
- 摘要调用的用量纳入既有资源预算，并与普通工具循环轮次区分。

## Capabilities

### New Capabilities

- `history-compaction`: 自动、手动和工具循环中的安全上下文压缩，以及失败可见性。
- `session-persistence`: 可选会话存储、安全检查点、列表与恢复。

### Modified Capabilities

- `tool-loop`: 澄清摘要维护调用的资源计费和工具轮次语义。

## Impact

扩展 Prompt、Agent、REPL、Config 和现有 DAO；两种模型接口共用摘要调度，保持现有流式普通回答与工具协议。使用已有依赖，不引入新的 Cargo crate。OpenSpec 计划与用户可读计划均以中文维护。

## Non-goals

不实现跨模型或跨协议恢复、自动续跑中断工具、跨设备同步、向量记忆、分支会话或生产级事务协调。Trace 仍是独立的模型调用诊断数据。
