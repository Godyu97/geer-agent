# 桌面 GUI 宿主：模块定义

## 快速定位

用 Tauri 窗口、IPC 命令与桌面 HostAdapter 连接共用 React 前端和 ui/app。

- 代码：[src/ui/gui](../../../src/ui/gui)
- 代表性测试：[tests/gui_selection.rs](../../../tests/gui_selection.rs)、[src/ui/frontend/src/App.test.tsx](../../../src/ui/frontend/src/App.test.tsx)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

仅在 gui feature 下编译，处理窗口、Tauri plugin 与 IPC；Agent、事件缓存及授权在 ui/app，界面和协议在共享前端。
