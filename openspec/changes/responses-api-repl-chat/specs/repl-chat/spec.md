# Spec Delta

## MODIFIED Requirements

### Requirement: 流式多轮对话

系统 MUST 对每条非空普通输入向配置的模型发起流式聊天请求，并在收到文本片段时立即显示。默认 OpenAI 接口 MUST 为 Responses API；用户显式选择 Chat Completions 时 MUST 使用该接口。每次请求 MUST 包含当前会话内已完成的对话上下文；仅当一轮完整成功后，系统才 MUST 保存本轮回答。请求失败时，系统 MUST 显示错误、不保存失败轮次，并保持 REPL 可继续使用。

#### Scenario: 默认接口

- **WHEN** 用户没有指定 OpenAI API 类型且发送普通消息
- **THEN** 程序向 Responses API 发起流式请求

#### Scenario: 显式选择兼容接口

- **WHEN** 用户显式选择 Chat Completions 且发送普通消息
- **THEN** 程序向 Chat Completions API 发起流式请求

#### Scenario: 连续对话保留上下文

- **WHEN** 用户先后输入两条普通消息
- **THEN** 第二条请求包含第一条 user 消息及其完整 assistant 回答，并保留所选接口要求的相关上下文

#### Scenario: 流式显示文本

- **WHEN** 模型分多次返回文本片段
- **THEN** 每个非空片段在收到后立即显示，无需等待完整回答

#### Scenario: 请求失败后继续对话

- **WHEN** 模型请求或流读取失败
- **THEN** 程序显示错误、保留此前已完成的历史、不保存失败轮次，并重新等待输入

#### Scenario: 模型迟迟没有可显示回复

- **WHEN** 请求或流在限定时间内没有返回可显示文本
- **THEN** 程序显示超时错误、不保存该轮历史，并重新等待输入

#### Scenario: 模型返回空回复或中途断流

- **WHEN** 模型结束响应但没有可显示文本，或未报告完成便断开流
- **THEN** 程序显示失败原因、不保存该轮历史，并重新等待输入

#### Scenario: 重置所选接口的上下文

- **WHEN** 用户在任一接口完成一轮对话后输入 `/reset` 并再次发送消息
- **THEN** 后续请求不包含重置前的上下文

## ADDED Requirements

### Requirement: OpenAI API 类型配置

系统 MUST 允许用户通过可选配置项选择 Responses 或 Chat Completions；未设置时 MUST 使用 Responses。配置值无效时，系统 MUST 在进入 REPL 前说明错误并退出。系统 MUST 保持原有 API key、model 和 base URL 配置语义。

#### Scenario: 无效接口类型

- **WHEN** 用户把 API 类型配置为不支持的值
- **THEN** 程序在进入 REPL 前显示配置错误并退出
