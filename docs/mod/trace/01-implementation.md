# 调用 Trace：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 事件采集与脱敏

TraceCapture 收集请求/响应与工具执行记录，TraceRecord 归一化为可持久化记录并计算状态、用量和时间戳。redact_text/redact_json 在内容进入会话或 Trace 存储前移除配置中的敏感字符串；TraceStore 将记录写入选定数据库。记录失败由调用方处理为诊断或会话错误，不声称持久化成功。

## 验证入口与缺口

代表性测试：[src/dao/mod.rs](../../../src/dao/mod.rs)、[tests/tool_loop.rs](../../../tests/tool_loop.rs)、[tests/session_compaction.rs](../../../tests/session_compaction.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

src/dao/mod.rs 覆盖 TraceStore 路由与失败处理；tests/tool_loop.rs 覆盖各模型步骤、transport 重试、部分失败、写入失败告警和配置秘密脱敏；tests/session_compaction.rs 验证 Trace 与会话库共享配置的路径。本次未运行。外部数据库契约按专用测试 URL 条件执行，不能由 SQLite 或未配置的测试结果推断全部后端通过。

源代码入口：[src/trace](../../../src/trace)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
