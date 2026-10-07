# 工具注册与执行：模块定义

## 快速定位

注册 Agent 可调用的文件、查询、Bash、Web 与记忆工具，并实施参数验证、授权和输出限制。

- 代码：[src/tools](../../../src/tools)
- 代表性测试：[src/tools/tests.rs](../../../src/tools/tests.rs)、[tests/tool_loop.rs](../../../tests/tool_loop.rs)、[tests/process_safety.rs](../../../tests/process_safety.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

不依赖 provider、agent 或 UI；工具执行接收当前 workspace、配置 Bash 和授权回调。只读操作可并行，有副作用操作串行；外部页面和命令输出均视为不可信输入。
