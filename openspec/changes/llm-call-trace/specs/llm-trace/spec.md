# Spec Delta

## Purpose

为一次启动或重置后的对话提供稳定标识，并在用户显式选择数据库时保存每一步模型调用的完整请求与合并后的结果。调用记录让使用者能够按会话和请求定位流式响应、重试、用量及失败，同时保持未启用持久化时的普通对话体验。

## ADDED Requirements

### Requirement: 会话与调用标识

系统 MUST 在每次启动和每次 `/reset` 后产生新的 Session ID 并显示；同一 Session 内的模型步骤 MUST 各有独立 Request ID，同一步骤的重试 MUST 共用该 Request ID。

#### Scenario: 重置会话

- **WHEN** 用户在一次启动内发送多条消息，然后输入 `/reset` 再发送消息
- **THEN** 重置前的调用具有相同 Session ID，重置后的 Session ID 与之前不同，每个模型步骤都有独立 Request ID

#### Scenario: 模型步骤重试

- **WHEN** 一个模型步骤在获得有效响应前重试
- **THEN** 该步骤只有一个 Request ID，记录其总尝试次数

### Requirement: 可选的调用记录

系统 MUST 仅在用户显式选择 SQLite、PostgreSQL、MySQL 或 MongoDB 并提供连接地址时持久化调用记录；未选择数据库时 MUST 继续正常对话且不写入记录。无效的数据库配置 MUST 在启动时明确报错。

#### Scenario: 未选择数据库

- **WHEN** 用户没有配置 Trace 数据库
- **THEN** REPL 正常启动并显示 Session ID，模型调用不依赖数据库

#### Scenario: 无效配置

- **WHEN** 用户选择不支持的数据库，或者选择数据库但没有提供连接地址
- **THEN** 程序在进入交互前显示明确的配置错误

### Requirement: 完整模型步骤记录

启用持久化时，系统 MUST 对两种模型接口的每个模型步骤分别保存完整请求、合并后的响应、调用时间、耗时、尝试次数、模型、接口、可得的用量及状态；失败或超时时 MUST 保存已收到的内容和错误。记录 MUST 包含 Session ID、Request ID，并能关联现有的单次用户请求标识。记录 MUST 不包含 API key、HTTP 认证头或数据库连接凭据。

#### Scenario: 工具调用后继续请求模型

- **WHEN** 模型在一条用户输入中先请求工具，随后基于工具结果生成回答
- **THEN** 两次模型步骤分别持久化，具有不同 Request ID、相同 Session ID 和相同单次用户请求标识

#### Scenario: 流式响应中断

- **WHEN** 模型已返回部分文本或工具调用内容后请求失败
- **THEN** 对应调用记录标为失败，保留已收到的内容和错误

### Requirement: 一致的存取接口

系统 MUST 对四种可选数据库提供相同的单条与批量写入、按 Request ID 单条与批量读取、按 Session 分页读取能力。批量读取 MUST 按传入 ID 顺序返回结果；批量写入 MUST 给出逐项结果，无法确认已写入状态时 MUST 明确报告批次错误。

#### Scenario: 批量读取

- **WHEN** 读取方按指定顺序查询多个 Request ID，其中一个不存在
- **THEN** 返回结果与输入顺序一致，并明确标出不存在的记录

#### Scenario: 批量写入部分失败

- **WHEN** 一批记录中有部分记录因冲突或大小限制无法写入
- **THEN** 返回可确认的逐项结果；如果数据库故障令结果无法确认，则返回批次错误而不误报成功

### Requirement: 存储失败可见且不阻断模型

数据库连接、初始化或写入失败时，系统 MUST 显示不含连接凭据的告警，并继续现有模型调用和终端输出；模型请求本身的失败处理 MUST 保持原有行为。

#### Scenario: 数据库不可达

- **WHEN** 用户选择的数据库在启动时不可达
- **THEN** 用户看到 Trace 告警，仍能发起模型对话

#### Scenario: 写入途中失败

- **WHEN** 模型步骤已完成，但对应 Trace 写入失败
- **THEN** 用户看到包含 Request ID 的告警，模型回答仍正常呈现
