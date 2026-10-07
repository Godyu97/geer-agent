# 模型协议适配：模块定义

## 快速定位

为 Agent 提供统一 ChatProvider 接口，适配 OpenAI Responses 与 Chat Completions 流式调用。

- 代码：[src/provider](../../../src/provider)
- 代表性测试：[tests/responses_retry.rs](../../../tests/responses_retry.rs)、[tests/tool_loop.rs](../../../tests/tool_loop.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

只依赖 config、provider 契约、serde 和 trace 捕获；不负责工具执行或会话持久化。API 选择、端点与凭据由 Config 提供。
