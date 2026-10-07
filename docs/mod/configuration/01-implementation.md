# 配置与路径：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 配置加载与验证

src/config 从进程环境和 dotenv 来源组装 Config，校验模型 API、资源预算、数据库种类与 URL、持久化开关、工具设置和 Bash 可执行文件；缺少必需模型键或非法值会返回错误。UiMode 解析界面选择；config/path 归一化 Windows/MSYS 路径，config/process 为后台子进程设置统一行为。WebConfig 只在 Web 启动时读取端口和 token。

## 验证入口与缺口

代表性测试：[tests/config_paths.rs](../../../tests/config_paths.rs)、[tests/process_safety.rs](../../../tests/process_safety.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

tests/config_paths.rs 验证配置目录、优先级和路径行为；tests/process_safety.rs 覆盖相关子进程约定。另有 src/config 内单元测试。本次未运行。

源代码入口：[src/config](../../../src/config)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
