# 调用 Trace：模块定义

## 快速定位

采集模型、工具、用量和错误事件，并提供脱敏、状态与记录结构。

- 代码：[src/trace](../../../src/trace)
- 代表性测试：[src/dao/mod.rs](../../../src/dao/mod.rs)、[tests/tool_loop.rs](../../../tests/tool_loop.rs)、[tests/session_compaction.rs](../../../tests/session_compaction.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

不发起网络请求；Provider/Agent 写入 TraceCapture，dao::TraceStore 负责持久化。任何凭据或端点敏感部分通过 redaction 处理，日志/诊断不要输出密钥。
