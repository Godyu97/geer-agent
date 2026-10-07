# 桌面 GUI 宿主：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 窗口与 IPC

src/ui/gui/mod.rs 注册 Tauri 插件、IPC 命令与窗口关闭事件；GuiBridge 创建 Host::Desktop 的 AppRuntime，维护单窗口 client，并把 connect/submit/authorize/close 转换成 runtime 调用。窗口关闭请求先交给共享关闭流程，待保存后退出；强制关闭命令会取消授权。桌面前端通过 hosts/desktop.ts 实现 HostAdapter。

## 验证入口与缺口

代表性测试：[tests/gui_selection.rs](../../../tests/gui_selection.rs)、[src/ui/frontend/src/App.test.tsx](../../../src/ui/frontend/src/App.test.tsx)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

tests/gui_selection.rs 覆盖桌面模式选择；前端共享组件测试在 src/ui/frontend/src。本次未运行，GUI 构建还需前端产物与平台 Tauri 依赖。

源代码入口：[src/ui/gui](../../../src/ui/gui)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
