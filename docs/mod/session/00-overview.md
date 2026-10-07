# 会话与工作区：模块定义

## 快速定位

维护 workspace、会话运行状态、快照/事件恢复、历史列表和安全删除预览。

- 代码：[src/session](../../../src/session)
- 代表性测试：[tests/session_compaction.rs](../../../tests/session_compaction.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

SessionRuntime 调用 DAO 保存并检测 revision 冲突；SessionManager 管理当前会话和 workspace。删除需经过预览与确认，加载会话要校验模型/API/端点兼容及事件链。
