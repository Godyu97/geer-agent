# Spec Delta

## Purpose

让模型在每轮对话中了解本机操作系统与实际 Bash 版本，从而给出适合当前环境的命令建议；这些默认上下文随请求发送，在清空会话后也继续可用，不依赖用户重复描述运行环境。

## ADDED Requirements

### Requirement: 默认系统提示与环境信息

系统 MUST 在每次模型请求中提供简短的本地助手说明和 `<context_data>`，其中包含系统版本与所选 Bash 版本。Linux 的系统版本 MUST 包含发行版和内核，Windows 的系统版本 MUST 包含 Windows 版本；系统版本无法探测时 MUST 明确标为未知。

#### Scenario: 两种模型接口

- **WHEN** 用户选择任一种受支持的模型接口并发送消息
- **THEN** 请求包含同一份默认系统提示及当前环境版本信息

#### Scenario: 重置会话

- **WHEN** 用户输入 `/reset` 后再次发送消息
- **THEN** 新请求不含重置前的对话，但仍包含默认系统提示及环境版本信息
