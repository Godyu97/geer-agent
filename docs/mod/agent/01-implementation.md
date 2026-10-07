# Agent 编排：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 消息处理与工具循环

Agent 收到用户消息后刷新 workspace 根 AGENTS.md 指令，并按配置读取可用记忆/主动召回，再让 provider 基于 Prompt 生成回应或工具调用。工具结果回到模型继续下一轮，流式文本和进度通过回调输出。循环受 turn、tool-call、时长、token、成本限制，并用 LoopGuard 阻止重复调用、连续错误和无进展；完成或失败时记录 Trace 并按检查点保存会话。

## 验证入口与缺口

代表性测试：[tests/tool_loop.rs](../../../tests/tool_loop.rs)、[tests/session_compaction.rs](../../../tests/session_compaction.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

tests/tool_loop.rs 覆盖模型工具循环；tests/session_compaction.rs 覆盖跨进程保存、恢复与压缩路径。按 README 使用受限 Make 入口；本次未运行。

源代码入口：[src/agent](../../../src/agent)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
