# Design

## Context

现有入口是 `repl::run()`：加载配置、构造 `Provider` 与 `Tools`，在同一个循环里读输入、流式打印、执行 `/reset`。`ChatProvider::stream_reply` 接收 `&mut Tools`，Chat Completions 与 Responses 两个分支各自在协议内部跑最多五轮工具循环。`tools` 为了给模型说明书，直接依赖 `async-openai` 的 Chat / Responses 工具类型。

See proposal.md - Why。行为不变（skip_specs）。

## Goals / Non-Goals

**Goals:**

- 把依赖收成单向分层，让后续能力加在正确的模块上。
- 工具循环从「协议内部」搬到 `agent`，两种 API 只负责单步请求和本协议的历史提交。
- `tools` 不再知道 OpenAI 请求类型；`repl` 不再构造工具。

**Non-Goals:**

- 不改流式超时、重试、授权、工具结果文本。
- 不新增 Session 框架 crate，不用 trait object 擦掉泛型回调。
- 不保留一套无工具的第二入口；`repl` 允许引用 `provider`，当前不必再做闲置的 `ChatSession`。

## Decisions

- **模块图（只允许实线方向）**

```
  config
   ^  ^  ^  ^
   |  |  |  |
provider  tools
   ^        ^
   |        |
  repl      |
   ^        |
   |        |
 agent------+
   ^
   |
  main
```

  `config` 不引用其它业务模块。`provider` 与 `tools` 互不引用：说明书用一份与协议无关的 `ToolSpec`（名字、描述、JSON Schema），由 `agent` 从 `tools` 拷到 `provider`。不把 `ToolSpec` 放进 `config`（它不是运行配置）；也不为这个三字段结构再开模块。

- **`repl` 只做 Read-Eval-Print-Loop。** 抽出 `Session`：`handle_message` + `reset`。循环、`/help` `/reset` `/exit`、提示符和按行首着色仍留在 `repl`（教程把颜色集中在一处；工具进度行仍以 `\n[调用工具` / `\n[工具调用轮次` 为启发式，避免再引入 `DeltaKind`）。不把 `Session` 做成对象安全 trait：`on_delta` 继续是 `FnMut`，与现有流式回调一致。

- **`provider` 改为单步 API，不再执行工具。** 两种协议的历史提交时机不同（Chat 立刻 push；Responses 先放 `pending` 再 commit），所以 trait 覆盖这一差异，而不是让 `agent` 了解 `InputItem` / `ChatCompletionRequestMessage`：

  - `begin_turn`：登记本轮用户输入
  - `complete_step`：带上 `ToolSpec` 发一次流式请求，返回文本和工具调用；不改已提交历史
  - `apply_tool_results`：把「助手工具调用 + 执行结果」写入本协议历史
  - `finish_turn`：无工具调用时提交助手文本
  - `commit_turn` / `rollback_turn`：出错或轮次用尽时，按「本轮是否已执行工具」提交或丢弃（与现有语义一致）
  - `reset`

  不把循环留在 `provider` 再注入 `ToolExecutor`：那样工具编排仍藏在协议里，`agent` 名不副实。也不为两种 API 再做统一的 history 类型：零转换仍是教程原则，转换只发生在各自分支内部。

- **`agent` 持有 `Provider` + `Tools`，实现 `repl::Session`。** 工具循环（最多五轮、`[调用工具 …]` 进度、轮次用尽提示、失败时 commit/rollback）只写一份，用 `ChatProvider` 泛型覆盖两个协议。`MAX_TOOL_ROUNDS` 从 `tools` 挪到这里。入口改为 `agent::run()`：加载 `Config`，再交给 `repl::run`。

- **所有权。** `Agent` 拥有 provider 与 tools；`repl::run` 借 `&mut impl Session`。`complete_step` 的 `on_delta` 仍由 REPL 传入，agent 只在工具进度处额外回调同一闭包。Responses 把当前轮 `pending` / `last_output` 收进结构体字段，因为循环不再是单个 `stream_reply` 的局部变量。

- **错误处理。** 协议不完整（缺工具名 / call id）仍由对应 `complete_step` 返回错误；是否提交历史由 agent 按 `used_tools` 决定，与现在两个 `stream_reply` 的分支一致。工具执行失败继续作为结果字符串回传，不升格成 REPL 错误。

- **不选的方案。** 不让 `repl` 直接收 `Option<Tools>`（基底会依赖工具）。不把 `tools` 实现 `provider` 里的 trait（会迫使 `tools` 依赖 `provider`）。不复制一套无工具 `repl::run` 给 `main` 走（违反「一个入口」）。

## Risks / Trade-offs

- [单步 trait 比原来的 `stream_reply` 宽] → 方法名与现有 Chat 的 `begin_turn` / `finish_turn` / `rollback_turn` 对齐，Responses 补齐对称方法；用注释写清「Chat 立刻写入、Responses 延迟提交」。
- [repl 仍用字符串前缀给工具行上色] → 显示约定不变；若以后进度文案改了，只改 agent 与这一处启发式。
- [内部重构误伤历史提交] → 现有 `tests/tool_loop.rs` 与 `tests/responses_retry.rs` 钉住工具对、失败保留、五轮上限和重试；agent 用假 `ChatProvider` 钉住循环的 commit/rollback 分支。

## Migration Plan

纯模块搬迁，无数据格式与配置迁移。行为由现有集成测试守护；若回归，按文件回退 `src/provider`、`src/repl`、`src/tools` 并去掉 `src/agent/`。
