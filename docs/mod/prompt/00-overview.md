# Prompt 与上下文：模块定义

## 快速定位

组装模型对话上下文、项目指令、会话快照和压缩所需数据。

- 代码：[src/prompt](../../../src/prompt)
- 代表性测试：[tests/session_compaction.rs](../../../tests/session_compaction.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

由 Agent 调用、provider 消费；会话持久化快照不应包含仅用于运行环境的凭据。压缩后的摘要与原始事件历史职责不同。
