# 界面中立交互契约：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 命令解析与执行

command.rs 将用户输入解析成消息、会话、workspace、历史删除、记忆和诊断等命令；execute 将需要修改会话的操作分派给 Session trait。删除与清空先生成预览，用户确认后按预览中的唯一 ID 执行；消息由 Session 处理并经回调逐段发送输出。 REPL/TUI 和图形 runtime 复用这些语义，具体确认 UI 由调用方提供。

## 验证入口与缺口

代表性测试：[src/interaction/command_tests.rs](../../../src/interaction/command_tests.rs)、[src/interaction/tests.rs](../../../src/interaction/tests.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

src/interaction/command_tests.rs 和 tests.rs 验证解析、执行、确认及失败路径；测试模块在源码目录内。本次未运行。

源代码入口：[src/interaction](../../../src/interaction)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
