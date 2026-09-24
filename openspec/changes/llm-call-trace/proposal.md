# Proposal

## Why

现在只能从终端看到每条输入的 Agent 运行摘要和工具摘要，无法按会话追查实际发送给模型的内容、模型返回了什么，以及失败发生在哪一步。新增可选的持久化 Trace，使学习和排查流式调用时有完整依据。

## What Changes

- 启动和 `/reset` 后分别产生新的 Session ID，并在终端显示；同一会话内的模型调用可按此 ID 关联。
- 每个模型步骤产生独立 Request ID；步骤内重试归入同一条调用记录。
- 显式配置数据库后，保存模型请求、合并后的完整响应或失败前已收到的内容，以及时间、耗时、尝试次数、用量和错误。
- 提供一致的单条、批量写入与读取能力，可从 SQLite、PostgreSQL、MySQL、MongoDB 中选择一种；数据库故障会告警，模型调用仍继续。
- 未配置数据库时不持久化 Trace；原有工具摘要仍不包含工具参数和结果原文。

## Capabilities

### New Capabilities

- `llm-trace`: Session 与 Request 标识、可选的模型调用持久化、批量存取和失败可见性。

### Modified Capabilities

无。

## Impact

涉及 REPL 的 Session 显示、Agent 与两种模型接口的调用边界、新增 Trace 和 DAO 模块、配置与使用文档。新增 SQL ORM、MongoDB 驱动、UUID 与序列化依赖；数据库连接串和对话内容都属于敏感数据。

## Non-goals

本次不增加 REPL Trace 查询命令、自动清理策略、会话恢复、数据库间同步或原始 SSE 事件归档。
