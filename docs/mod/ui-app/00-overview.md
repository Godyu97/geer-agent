# 图形共用运行时：模块定义

## 快速定位

为 GUI/Web 共用 Agent 工作线程、命令队列、事件快照、授权 Gate 与关闭保存流程。

- 代码：[src/ui/app](../../../src/ui/app)
- 代表性测试：[tests/gui_selection.rs](../../../tests/gui_selection.rs)、[tests/web_ui.rs](../../../tests/web_ui.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

图形 UI 唯一组合 Agent 的工作线程位于 ui/app；宿主通过 Host/事件 sink 接入，不复制 Agent 或会话模型。UI 会话写操作经 interaction::execute。
