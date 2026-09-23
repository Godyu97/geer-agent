# Spec Delta

## Purpose

允许用户明确选择本机 Bash 可执行文件，并让命令执行使用这一选择；启动时核对解释器与版本，避免模型收到的环境信息和实际运行的 Bash 不一致，也让配置错误在进入交互前可见。

## ADDED Requirements

### Requirement: Bash 可执行文件配置

系统 MUST 接受可选的 `GEER_AGENT_BASH_BIN` 绝对路径配置，使用所选 Bash 执行已授权命令；未设置或为空时 MUST 从进程 `PATH` 查找 `bash`。

#### Scenario: 自定义 Bash

- **WHEN** 用户提供可运行的 Bash 绝对路径并授权 Bash 工具
- **THEN** 系统使用该可执行文件运行命令

#### Scenario: 默认 Bash

- **WHEN** 用户未提供 Bash 路径并授权 Bash 工具
- **THEN** 系统使用进程 `PATH` 找到的 `bash` 运行命令

### Requirement: 启动时验证 Bash

系统 MUST 在进入交互前获取所选 Bash 的版本；路径无效、无法运行或无法识别 Bash 版本时 MUST 显示明确错误并退出。

#### Scenario: Bash 路径无效

- **WHEN** 所选 Bash 路径无效或不可运行
- **THEN** 系统在进入交互前显示包含 Bash 配置信息的错误并退出

#### Scenario: 无法识别 Bash 版本

- **WHEN** 所选程序的版本输出不是可识别的 Bash 版本
- **THEN** 系统在进入交互前显示版本探测错误并退出
