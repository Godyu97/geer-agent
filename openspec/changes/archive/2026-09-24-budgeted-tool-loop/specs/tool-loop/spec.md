# Spec Delta

## MODIFIED Requirements

### Requirement: 工具调用循环

系统 MUST 在两种已支持的模型接口中声明可用工具，收集完整调用后执行，并使用对应调用标识回传结果；每个 Assistant 响应 MUST 只计为一个 Agent Turn，每个实际执行的工具调用 MUST 独立计数，系统 MUST 继续循环直到模型产生最终回答或执行预算要求收敛。

#### Scenario: 查询当前时间

- **WHEN** 模型请求当前时间工具
- **THEN** 系统返回真实本地时间，模型可依据结果继续回答

#### Scenario: 单个 Turn 包含多个调用

- **WHEN** 模型在一个 Assistant 响应中返回三个工具调用
- **THEN** 系统记录一个 Agent Turn 和三个 Tool Call，按现有顺序逐一执行、配对结果并继续向模型请求

#### Scenario: 连续调用超过旧上限

- **WHEN** 同一用户输入已完成五个包含工具调用的 Agent Turn、模型仍请求工具且各项预算仍有余量
- **THEN** 系统继续执行调用，而不是以旧的五轮限制终止任务

## ADDED Requirements

### Requirement: 工具执行预算与收敛

系统 MUST 对每次用户请求应用独立预算，默认软 Turn 上限为 12、硬 Turn 上限为 30、实际 Tool Call 上限为 100；软上限用于向模型提供收敛提示，任一硬上限用于停止工具执行并在协议层关闭工具后请求最佳可用最终回答。最终回答 MUST 基于已有上下文，并明确任何未完成或未验证的工作。

#### Scenario: 达到软 Turn 上限

- **WHEN** 同一用户请求已经完成 12 个 Agent Turn 且尚未自然结束
- **THEN** 系统在下一次模型请求中加入预算提示，要求优先完成必要工作、避免可选探索

#### Scenario: Turn 预算接近耗尽

- **WHEN** 同一用户请求的剩余 Turn 不多于三个
- **THEN** 系统向模型说明准确的剩余 Turn，并随余量减少强化立即收敛的要求

#### Scenario: 硬 Turn 上限触发 Finalization

- **WHEN** 同一用户请求已经完成 29 个 Agent Turn 且仍未产生最终回答
- **THEN** 系统将第 30 个且最后一个允许的 Assistant 响应用于 Finalization，不声明或执行任何工具，并要求模型仅使用已有上下文回答

#### Scenario: Tool Call 批次超过剩余预算

- **WHEN** 执行当前 Assistant 响应中的完整工具批次会使实际 Tool Call 总数超过 100
- **THEN** 系统不执行该批次中的任何调用，停止提供工具，并在剩余 Turn 内进入 Finalization

#### Scenario: Tool Call 预算恰好耗尽

- **WHEN** 一个完整工具批次执行后实际 Tool Call 总数达到 100 且模型尚未给出最终回答
- **THEN** 系统不再发起可调用工具的模型请求，并在剩余 Turn 内进入 Finalization

#### Scenario: Finalization 无法产生有效文本

- **WHEN** 无工具的 Finalization 请求失败或仍返回工具调用而没有可用最终文本
- **THEN** 系统不执行该调用，保留已经完成的调用与结果，并向用户明确显示预算已耗尽及回答未完成
