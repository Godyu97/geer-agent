# 终端界面：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## REPL 与 TUI 路径

REPL 循环读取一行，处理公共命令或把用户消息交给 Session，并经回调显示流式文本、工具进度和错误。TUI 使用 Crossterm/Ratatui 管理键盘输入、消息视图以及会话/记忆面板，通过 interaction 执行写操作；终端 Raw/Alternate 屏幕生命周期由 Tui 的恢复逻辑负责。交互终端是 auto 默认选择，非终端管道进入 REPL。

## 验证入口与缺口

代表性测试：[src/ui/mod.rs](../../../src/ui/mod.rs)、[src/interaction/tests.rs](../../../src/interaction/tests.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

TUI 输入、会话/记忆面板有模块内单元测试；界面分派测试在 src/ui/mod.rs。本次未运行。

源代码入口：[src/ui/commands.rs](../../../src/ui/commands.rs)、[src/ui/repl](../../../src/ui/repl)、[src/ui/tui](../../../src/ui/tui)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
