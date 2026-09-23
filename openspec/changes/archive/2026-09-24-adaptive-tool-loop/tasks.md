# Tasks

## 1. 第二阶段：工具执行与空转保护

- [x] 1.1 为工具结果加入可靠的成功状态，并标记只读/顺序执行模式；用文件错误、Bash 非零退出、授权拒绝和成功结果测试验证，运行 `cargo test tools::`。
- [x] 1.2 在一个 Assistant Turn 内并行执行相邻已授权只读调用，顺序执行 Bash/write/edit，并按原次序回传结果；用读写读混合批次及多读取测试验证，运行 `cargo test agent::` 与 `cargo test --test tool_loop`。
- [x] 1.3 记录规范化参数与结果摘要、连续相同结果及连续工具错误；第三次提示、第四次无工具收敛，并验证成功修改打断重复测试，运行 `cargo test agent::`。

## 2. 第三阶段：进展与资源预算

- [x] 2.1 跟踪每个工具 Turn 的新读取、成功修改和新结果；连续三次无进展提示、第四次收敛；有进展时动态推迟通用软提示，但不放宽硬上限，运行 `cargo test agent::`。
- [x] 2.2 从 Chat Completions 与 Responses 流式响应提取可选 Token 用量；验证有用量、缺失用量和尾包情况，运行 `cargo test provider::`。
- [x] 2.3 加入可选 Token/费用上限及其环境配置校验、默认总时长上限；触及上限或配置上限却缺用量时停止继续工具并进入 Finalization，运行 `cargo test config::`、`cargo test agent::`。
- [x] 2.4 记录每次请求与工具的结构化本地指标和结束原因；验证不输出参数、结果原文或密钥，运行 `cargo test agent::` 和 `cargo test --test tool_loop`。

## 3. 整体验收

- [x] 3.1 调整两种协议的模拟服务，覆盖并行批次、连续失败、重复结果、进展恢复及资源耗尽后的无工具收敛；运行 `cargo test --test tool_loop`。
- [x] 3.2 运行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate adaptive-tool-loop --strict`；记录未归档基线对校验的影响。
