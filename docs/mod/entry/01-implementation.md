# 程序入口与界面分派：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 启动与界面选择

main 只调用 ui::run。入口加载环境并解析 GEER_AGENT_UI；auto 在 stdin/stdout 均为终端时选 TUI，否则选 REPL。显式 TUI 要求交互终端；gui/web 未编译相应 feature 时返回明确错误。终端路径创建 current-thread Tokio runtime 与 Agent 后进入 REPL/TUI；图形宿主把 Agent 建立交给 ui/app 工作线程。界面解析与 auto 选择的纯逻辑在 ui/mod.rs 有单元测试。

上述 auto 选择用于普通构建；`desktop-gui` 含 `gui`，auto/gui 始终选择 GUI，显式 repl/tui/web 返回模式错误。桌面启动配置错误也进入 GUI 的可展示错误路径，不依赖不可见控制台。判定来源是 [resolve_mode 与 run](../../../src/ui/mod.rs)，不是启动时是否有桌面环境的猜测。

## 验证入口与缺口

代表性测试：[src/ui/mod.rs](../../../src/ui/mod.rs)、[tests/gui_selection.rs](../../../tests/gui_selection.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

入口分派测试内嵌在 src/ui/mod.rs；默认构建可用 cargo build，图形 feature 构建须先准备前端。本次未运行。

源代码入口：[src/main.rs](../../../src/main.rs)、[src/ui/mod.rs](../../../src/ui/mod.rs)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
