# 浏览器 Web 宿主：模块定义

## 快速定位

用 Axum 提供登录、静态资源、同源 HTTP/WebSocket 与共享 UI 事件传输。

- 代码：[src/ui/web](../../../src/ui/web)
- 代表性测试：[tests/web_ui.rs](../../../tests/web_ui.rs)、[src/ui/web/auth.rs](../../../src/ui/web/auth.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

仅在 web feature 下编译；共享 Agent/runtime 在 ui/app。浏览器凭证进程内保存，模型密钥不传给浏览器；输入、帧和并发连接有界。
