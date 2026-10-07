# 终端界面：模块定义

## 快速定位

提供按行交互的 REPL 与全屏 TUI，统一呈现消息、工具进度、会话和记忆操作。

- 代码：[src/ui/commands.rs](../../../src/ui/commands.rs)、[src/ui/repl](../../../src/ui/repl)、[src/ui/tui](../../../src/ui/tui)
- 代表性测试：[src/ui/mod.rs](../../../src/ui/mod.rs)、[src/interaction/tests.rs](../../../src/interaction/tests.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

终端界面负责输入、渲染和确认；命令语义来自 interaction。REPL/TUI 不直接操作持久化适配，TUI 需在退出时恢复终端状态。
