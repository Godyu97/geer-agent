# Proposal

## Why

当前 Bash 工具只能从 `PATH` 查找解释器，无法指定另一份 Bash；模型也不知道实际运行环境的系统和 Bash 版本，可能建议不适用的命令。

## What Changes

- 允许通过可选配置指定 Bash 可执行文件；默认继续从 `PATH` 查找。
- 启动时确认所选 Bash 可运行并获取版本，失败时明确报错。
- 每次模型请求带简短的默认系统提示，其中 `<context_data>` 包含系统版本和 Bash 版本；重置对话后仍提供。

## Capabilities

### New Capabilities

- `bash-tool`：补充 Bash 可执行文件的选择和启动验证。
- `repl-chat`：补充默认系统提示及环境信息。

### Modified Capabilities

无；主规格尚未归档，沿用已有变更使用的能力 id。

## Non-goals

不加入用户自定义 prompt、多解释器支持或新的工具授权模式。

## Impact

涉及配置读取、Bash 工具、两种 OpenAI 请求、示例环境文件和 README；无需新依赖。
