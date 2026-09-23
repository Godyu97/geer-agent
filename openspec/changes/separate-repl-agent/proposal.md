# Proposal

## Why

当前 REPL 同时负责终端输入输出、配置加载、模型会话和工具状态；`provider` 又直接依赖 `tools` 跑工具循环。基础交互、模型协议和带工具的 Agent 缠在一起，后续加压缩、会话、权限时会继续把能力塞进错误的层。趁工具能力仍在增量建设，按「config 共享、repl 为基底、agent 带工具」把依赖方向说清。

## What Changes

- `cargo run` 仍启动同一个交互程序；提示、命令、流式回复、工具调用和错误显示保持现有行为。
- 关闭工具时仍可普通聊天；`/reset` 仍清空对话及工具授权。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

无。本变更仅调整内部结构，使用 `skip_specs: true`。

## Non-goals

- 不新增工具、命令、配置项或可切换的第二套运行模式。
- 不改动现有模型协议、工具授权范围与会话历史语义。
- 不引入 Agent 框架，不为依赖方向增加新 crate。

## Impact

整理模块边界与允许的引用方向：

- `config`：配置层，可被所有业务模块引用。
- `provider`：模型协议，可被 `repl` / `agent` 引用；不再引用 `tools`。
- `repl`：无工具的交互基底（读入、命令、着色、循环）；不引用 `tools` / `agent`。
- `tools`：工具定义与执行；不引用 `provider` / `repl` / `agent`。
- `agent`：主业务，带工具的 REPL；引用 `repl` 与 `tools`，并编排 `provider` 的单步请求。
- `main` 只启动 `agent`。

沿用现有依赖，不增加 crate。现有进行中的工具变更及未提交代码保持行为原状，实施时在其上做最小迁移。
