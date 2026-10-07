# 构建入口：模块定义

## 快速定位

按 Cargo feature 追踪 GUI/Web 前端产物，检查 Web 入口并调用可选 Tauri 构建。

- 代码：[build.rs](../../../build.rs)
- 验证入口：[01 的构建验证](01-implementation.md#验证入口与缺口)，未发现独立 build.rs 单元测试
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

只负责构建期资源与 Tauri 配置，不实现运行时业务逻辑。资源来自 src/ui/frontend 的构建输出；feature 和生成边界见 Cargo.toml 与 Makefile。
