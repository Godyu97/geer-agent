# Tasks

## 1. 建立预算状态与临时提示

- [x] 1.1 在现有 `agent` 模块增加默认 `AgentBudget`、每轮 `AgentMetrics` 和边界判断，分别记录 Assistant Turn 与实际 Tool Call；用 11/12、27/28/29/30 Turn 及 99/100 Tool Call 的单元测试验证计数和剩余预算，并运行 `cargo test agent::`。
- [x] 1.2 扩展 `Prompt` 的消息快照以接收可选静态运行时指令，让 Chat Completions 的 system 内容和 Responses 的 instructions 在当次请求中包含提示但不写入历史；用两种协议的单元测试验证下一轮快照不残留旧提示，并运行 `cargo test prompt::`。

## 2. 接入预算驱动循环

- [x] 2.1 将现有五轮 `for` 循环增量改为预算状态驱动的持续循环：每个模型响应计一个 Turn，一个响应中的调用按实际数量计数并继续顺序执行，超过旧五轮但预算充足时不停；补充多调用、六轮继续和自然回答的测试，并运行 `cargo test agent::`。
- [x] 2.2 在执行工具前原子检查完整批次，保证会超过 100 次时整批不执行、恰好达到 100 次后不再提供工具，同时保持已完成 call/result 对和现有错误提交/回滚语义；补充 99 加 2、99 加 1及工具后请求失败测试，并运行 `cargo test agent::`。
- [x] 2.3 接入软预算与剩余 Turn 提示，并把第 30 个总 Turn 实现为工具清单为空的 Finalization；覆盖 12 Turn 提示、剩余 3 Turn、无工具最终回答、Finalization 仍返回调用及请求失败的本地兜底测试，并运行 `cargo test agent::`。

## 3. 双协议回归与质量门

- [x] 3.1 更新 `tests/tool_loop.rs` 的模拟服务，分别验证 Chat Completions 与 Responses 在第六个工具 Turn 后继续、Finalization 请求不含工具定义且带收敛指令、最终文本进入后续历史；运行 `cargo test --test tool_loop`。
- [x] 3.2 运行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate budgeted-tool-loop --strict`，确认无新增 Clippy 警告、普通对话/工具授权/Responses 重试回归通过，并记录任何仅因未归档 `tool-loop` 基线造成的 OpenSpec 校验边界。
