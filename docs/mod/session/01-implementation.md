# 会话与工作区：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 检查点保存与恢复

SessionRuntime 把 Prompt 新事件排入带 parent_id 的事件链，并保存包含快照、workspace、模型/API/端点身份与 revision 的记录；成功后清空待写队列，失败保留待补写状态，revision 不匹配则报告冲突。恢复时校验端点兼容、workspace 路径、快照格式和事件链后重建 Prompt。SessionManager 提供会话切换、列表、标题与删除预览；预览确认后再执行批量删除。

## 验证入口与缺口

代表性测试：[tests/session_compaction.rs](../../../tests/session_compaction.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

tests/session_compaction.rs 覆盖跨进程持久化、恢复、压缩和历史删除；DAO 适配测试覆盖并发 revision。本次未运行。

源代码入口：[src/session](../../../src/session)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
