# 记忆检索排序：模块定义

## 快速定位

将长期记忆切成重叠片段，并对查询进行多语言词项提取和 BM25-lite 排序。

- 代码：[src/retrieval.rs](../../../src/retrieval.rs)
- 代表性测试：[src/retrieval.rs](../../../src/retrieval.rs)、[src/memory/tests.rs](../../../src/memory/tests.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

纯检索逻辑，不访问数据库或网络；MemoryService 负责提供记录、token 预算和最终选择。
