# 持久化适配：模块定义

## 快速定位

为会话、Trace 与长期记忆提供 SQL 和 MongoDB 存储接口及迁移。

- 代码：[src/dao](../../../src/dao)
- 代表性测试：[src/dao/mod.rs](../../../src/dao/mod.rs)、[tests/session_compaction.rs](../../../tests/session_compaction.rs)、[src/memory/tests.rs](../../../src/memory/tests.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

DAO 不依赖 Agent 或 UI；调用者通过 SessionStore、TraceStore、MemoryStore 使用统一接口。SQL 迁移和 MongoDB 集合/索引逻辑只在对应适配中维护。
