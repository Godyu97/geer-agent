# 长期记忆：模块定义

## 快速定位

管理跨会话长期记忆的读写、精确去重、编辑删除、关键词搜索和主动召回。

- 代码：[src/memory](../../../src/memory)
- 代表性测试：[src/memory/tests.rs](../../../src/memory/tests.rs)、[src/tools/tests.rs](../../../src/tools/tests.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

MemoryService 通过 DAO 持久化；Agent 与 Tools 共享服务。记忆独立于 workspace/session，UI 写操作经 interaction；不可用状态必须与空列表区分。
