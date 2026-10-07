# 图形共用运行时：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 工作线程与事件同步

AppRuntime 在专用线程创建 Tokio current-thread runtime 和 Agent，将图形宿主命令串行排队并转交 interaction。EventHub 保存当前 snapshot、流式输出、运行状态、授权与诊断，客户端连接时先获得完整状态，后续事件按 revision 更新。授权 Gate 独立于命令队列，因此模型/工具等待确认时 UI 回复仍可唤醒工作线程。关闭会取消待决授权，等待正在运行命令结束并保存会话；失败会返回可展示的关闭错误。

## 验证入口与缺口

代表性测试：[tests/gui_selection.rs](../../../tests/gui_selection.rs)、[tests/web_ui.rs](../../../tests/web_ui.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

src/ui/app 内含事件、授权、关闭及命令测试；tests/gui_selection.rs 与 tests/web_ui.rs 覆盖宿主集成。本次未运行。

源代码入口：[src/ui/app](../../../src/ui/app)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
