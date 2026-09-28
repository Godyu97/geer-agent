# Spec Delta

## ADDED Requirements

### Requirement: 摘要调用的资源归属

系统 MUST 将摘要模型调用的实际 token、费用和耗时计入资源预算与运行记录；摘要调用 MUST 不计为普通工具循环的 Agent Turn，且 MUST 不声明或执行工具。资源预算不允许继续摘要时 MUST 保留原上下文并显示可见结果。

#### Scenario: 工具循环中的摘要

- **WHEN** 一批工具结果后触发摘要调用
- **THEN** 摘要用量计入本次请求的资源预算，实际工具次数和普通 Agent Turn 不因摘要增加

#### Scenario: 预算已耗尽

- **WHEN** 请求资源预算不足以继续摘要
- **THEN** 系统不执行额外工具或无界摘要调用，并按现有收敛路径提示用户

