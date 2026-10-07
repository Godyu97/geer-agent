# Agent 编排：模块定义

## 快速定位

管理一次对话中的模型轮次、工具调用、预算、循环保护、记忆召回与会话保存。

- 代码：[src/agent](../../../src/agent)
- 代表性测试：[tests/tool_loop.rs](../../../tests/tool_loop.rs)、[tests/session_compaction.rs](../../../tests/session_compaction.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

组合 provider、tools、prompt、session、memory、dao 与 trace；实现 interaction::Session，不引用具体 UI。工作区规则通过 Prompt 更新；数据适配由 DAO 提供。
