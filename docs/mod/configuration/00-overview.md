# 配置与路径：模块定义

## 快速定位

解析模型、UI、持久化、工具与资源配置，并集中处理 Bash/Windows 路径和后台子进程约定。

- 代码：[src/config](../../../src/config)
- 代表性测试：[tests/config_paths.rs](../../../tests/config_paths.rs)、[tests/process_safety.rs](../../../tests/process_safety.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

供所有模块引用，不依赖 provider/tools/agent/ui。配置值进入运行时前校验；错误需指出配置键但不得泄漏秘密。
