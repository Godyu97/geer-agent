# 模型协议适配：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 请求与流式响应

Provider 根据配置构造 Responses 或 Chat Completions 适配器，将 Prompt 消息和工具定义转换为对应 API 请求；流式增量传给 Agent 回调，完成后返回文本/工具调用/用量。协议事件被整理到 TraceCapture；可恢复的请求失败按对应策略重试，超时和协议错误以 Result 返回。

## 验证入口与缺口

代表性测试：[tests/responses_retry.rs](../../../tests/responses_retry.rs)、[tests/tool_loop.rs](../../../tests/tool_loop.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

tests/responses_retry.rs 锁定 Responses API 重试/错误语义，tests/tool_loop.rs 覆盖工具调用往返。本次未运行。

源代码入口：[src/provider](../../../src/provider)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
