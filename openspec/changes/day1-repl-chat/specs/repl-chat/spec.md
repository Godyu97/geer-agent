# Spec Delta

## Purpose

本能力为 GeekAgent 提供最小可运行的终端聊天入口，让用户能持续与模型对话，并通过每轮携带的消息历史维持当前进程内的上下文。它为后续逐步加入工具能力提供循环基础。

## ADDED Requirements

### Requirement: OpenAI 兼容配置

系统 MUST 从 `.env` 文件或进程环境变量读取 `OPENAI_API_KEY`、`OPENAI_MODEL` 和可选的 `OPENAI_BASE_URL`。进程环境变量 MUST 覆盖 `.env` 中的同名值；未提供 `OPENAI_BASE_URL` 时 MUST 使用 `https://api.openai.com/v1`。缺少 API key 或 model 时，系统 MUST 在进入交互前显示明确错误并退出。

#### Scenario: 使用环境变量配置

- **WHEN** 用户提供有效的 `OPENAI_API_KEY` 和 `OPENAI_MODEL`，且未指定 `OPENAI_BASE_URL`
- **THEN** 程序使用默认 OpenAI endpoint 并进入 REPL

#### Scenario: 缺少必需配置

- **WHEN** API key 或 model 缺失或为空
- **THEN** 程序说明缺少的配置并在进入 REPL 前退出

#### Scenario: 环境变量覆盖文件配置

- **WHEN** `.env` 与进程环境变量为同一配置项提供不同值
- **THEN** 程序使用进程环境变量中的值

### Requirement: 流式多轮对话

系统 MUST 对每条非空普通输入向配置的模型发起流式聊天请求，并在收到文本片段时立即显示。每次请求 MUST 携带当前会话内已完成的 user 与 assistant 消息；仅当一轮完整成功后，系统才 MUST 将 assistant 回答存入历史。请求失败时，系统 MUST 显示错误、移除该轮未完成的 user 消息，并保持 REPL 可继续使用。

#### Scenario: 连续对话保留上下文

- **WHEN** 用户先后输入两条普通消息
- **THEN** 第二条请求包含第一条 user 消息及其完整 assistant 回答

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

### Requirement: REPL 命令

系统 MUST 支持 `/help`、`/reset` 和 `/exit` 命令。`/reset` MUST 清空当前会话历史；`/exit` MUST 打印结束提示并退出；EOF MUST 正常结束进程。空行 MUST 不发起模型请求，未知斜杠命令 MUST 给出提示并继续等待输入。

#### Scenario: 查看帮助

- **WHEN** 用户输入 `/help`
- **THEN** 程序显示可用命令并继续等待输入

#### Scenario: 重置历史

- **WHEN** 用户输入 `/reset` 后再发送普通消息
- **THEN** 后续模型请求不包含 reset 前的会话消息

#### Scenario: 退出对话

- **WHEN** 用户输入 `/exit` 或终端输入结束
- **THEN** 程序正常退出而不发起模型请求

#### Scenario: 输入包含无效 UTF-8

- **WHEN** 终端输入的一行包含无效 UTF-8 字节
- **THEN** 程序提示重新输入、保持已有历史，并继续等待下一行

### Requirement: 终端视觉区分

系统 MUST 将用户提示标签显示为“李火旺🔥”，将模型提示标签显示为“Ai”。交互终端支持颜色且未禁用颜色时，系统 MUST 用不同颜色区分用户提示与模型回复；非交互输出或用户禁用颜色时，系统 MUST 不输出 ANSI 控制序列。

#### Scenario: 交互终端对话

- **WHEN** 用户在支持颜色的交互终端中对话
- **THEN** “李火旺🔥”提示与“Ai”回复以不同颜色显示

#### Scenario: 非交互输出或禁用颜色

- **WHEN** 输出被重定向，或用户设置 `NO_COLOR`
- **THEN** 提示和回复保持纯文本且标签不变
