# Prompt 与上下文：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 上下文组装与项目指令

Prompt 保存模型消息、原始事件与可恢复快照；ProjectInstructions 独立表示 workspace 根 AGENTS.md 内容，由 SessionManager 切换/恢复 workspace 时刷新，Agent 在每条消息前再刷新，并作为系统上下文提供。上下文逼近配置窗口时由 session/agent 发起压缩，保留近期对话和关键事实，同时维持工具调用配对；原始事件由会话事件链独立保存。

[ProjectInstructions::load](../../../src/prompt/mod.rs) 只读取 workspace 根指令；缺失/空文档视为无指令，其他读取错误返回失败。不会解析父/子目录规则或自动打开文档链接。根指令独立于会话快照和压缩，单条消息的工具续轮沿用本轮指令；对应目标与实施记录见 [add-project-memory](../../../openspec/changes/add-project-memory/specs/project-memory/spec.md)，不可把链接可达当作加载验证。

## 验证入口与缺口

代表性测试：[tests/session_compaction.rs](../../../tests/session_compaction.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

tests/session_compaction.rs 覆盖压缩、退出、恢复和继续对话；src/prompt 内有快照与格式相关测试。本次未运行。

源代码入口：[src/prompt](../../../src/prompt)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
