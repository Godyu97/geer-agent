# 程序入口与界面分派：模块定义

## 快速定位

从 main 进入 ui::run，加载配置并选择 REPL、TUI、GUI 或 Web。

- 代码：[src/main.rs](../../../src/main.rs)、[src/ui/mod.rs](../../../src/ui/mod.rs)
- 代表性测试：[src/ui/mod.rs](../../../src/ui/mod.rs)、[tests/gui_selection.rs](../../../tests/gui_selection.rs)
- 归属与代码覆盖：[catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
|---|---|
| 实现、状态、依赖、错误语义 | [01 当前实现](01-implementation.md)，先搜索相关标题 |
| 相关未完成事项与验证缺口 | [02 当前事项](02-issues.md)，只读命中项 |
| 命令、环境、跨模块约束 | [.ai 项目手册](../../../.ai/01-project.md) / [工程边界](../../../.ai/02-engineering.md) |

## 职责与契约边界

此处是应用组合点，可创建 Agent 并选择具体 UI；具体界面渲染与图形事件运行时分别位于 ui-terminal、ui-app、ui-gui、ui-web。
