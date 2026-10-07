# 浏览器 Web 宿主：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 认证、连接与关闭

WebConfig 提供监听端口与 token；登录成功后签发 HttpOnly、SameSite=Strict 的内存 Cookie，写请求与 WebSocket 校验 Origin/Host。静态文件从编译时嵌入的 dist/web 提供；WebSocket 连接接入 AppRuntime，命令提交和授权回复转为共享 runtime 操作。连接数量、事件队列与消息尺寸有限；Ctrl+C/SIGTERM 停止接收命令并等待 Agent 保存，慢请求在限定时间后中止。

当前默认监听 `0.0.0.0:8827`，来源是 [WebConfig](../../../src/config/web.rs) 与 [Web 宿主](../../../src/ui/web/mod.rs)；启动入口见 [项目手册](../../../.ai/01-project.md)。旧 verification 中的 9928、`make web` 等保留为当时验收证据，不能代替当前配置和 Make 目标。

## 验证入口与缺口

代表性测试：[tests/web_ui.rs](../../../tests/web_ui.rs)、[src/ui/web/auth.rs](../../../src/ui/web/auth.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

tests/web_ui.rs 覆盖认证、同源、事件、并发和关闭，编译条件是 Unix + web 且非 desktop-gui；src/ui/web/auth.rs 有凭证/Host 测试。本次未运行；构建前需 make frontend-build，端到端入口是 make web-test。

源代码入口：[src/ui/web](../../../src/ui/web)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
