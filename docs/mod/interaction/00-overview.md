# 界面中立交互契约：模块定义

## 快速定位

定义 Agent 可提供的 Session 能力、命令解析和多界面共用的会话/记忆操作。

- 代码：[src/interaction](../../../src/interaction)
- 代表性测试：[src/interaction/command_tests.rs](../../../src/interaction/command_tests.rs)、[src/interaction/tests.rs](../../../src/interaction/tests.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

不依赖具体 Agent 或 UI；Agent 实现 Session trait，各 UI/图形 runtime 经 execute 执行共用写命令。命令结果包含展示信息和需确认的预览，不直接控制具体窗口或终端。
