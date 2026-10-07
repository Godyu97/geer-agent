# 共用 React 前端：模块定义

## 快速定位

一套 React/TypeScript 应用用于桌面 GUI 和浏览器 Web，按构建模式注入不同宿主能力。

- 代码：[src/ui/frontend](../../../src/ui/frontend)
- 代表性测试：[App](../../../src/ui/frontend/src/App.test.tsx)、[reducer](../../../src/ui/frontend/src/model.test.ts)、[Web](../../../src/ui/frontend/src/Web.test.tsx)、[记忆面板](../../../src/ui/frontend/src/MemoryPanel.test.tsx)、[Markdown](../../../src/ui/frontend/src/markdown.test.ts)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

Bun 1.4.2 与 bun.lock 是唯一包管理入口。App、reducer、protocol、样式共用；Tauri IPC 和 WebSocket/HTTP 分别实现于 hosts/desktop.ts、hosts/web.ts，不复制第二份 App。
