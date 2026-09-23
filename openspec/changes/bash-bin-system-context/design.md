# Design

## Context

当前 `Command::new("bash")` 在执行时按 `PATH` 查找；配置层尚无 Bash 字段。两种模型接口各自构造请求，历史消息只保存对话内容。

## Goals / Non-Goals

**Goals:** 同一份启动时探测的 Bash 版本用于系统提示，工具执行使用同一 Bash 选择。

**Non-Goals:** 不加入可编辑的系统提示或跨平台系统信息框架。

## Decisions

- `Config` 保存 Bash 可执行文件选择。`GEER_AGENT_BASH_BIN` 非空时须为绝对路径；空值沿用 `bash`。启动时运行所选程序的 `--version` 并检查第一行的 Bash 标识，失败经 `Result` 传回入口；不靠首次工具调用才发现错误。路径由配置所有，工具持有自己的路径副本，避免生命周期借用穿过异步执行。
- 独立的 `prompt` 目录模块负责启动时探测 Bash 与系统信息、封装 `<context_data>` 和默认提示文本。Linux 读取发行版信息并执行 `uname`，Windows 执行 `cmd /C ver`，macOS 执行 `sw_vers`；失败时使用“未知”。子进程使用现有 Tokio 异步执行并设超时；不读取用户文件，也不增 crate。以后接入别的 LLM 仍复用同一组装结果。
- `agent` 将一次组装的提示传入 `Provider`，再由 Chat Completions 在每次请求临时放入 system 消息、Responses 在每次请求设置 `instructions`。系统提示不写入可重置的历史，故 `/reset` 只清对话，流式处理与失败回滚不变。比在各协议内分别组装更容易保持一致。
- 工具继续由现有 Tokio 子进程异步执行，超时与授权不变；版本探测仅在启动时运行并限制等待时间，避免卡住 REPL。配置仍遵循进程环境变量优先于 `.env` 的现有规则。

## Risks / Trade-offs

- [自定义路径指向非 Bash 程序] → 检查 `--version` 输出的 Bash 标识并在启动时失败。
- [系统版本探测受限] → prompt 明确标为未知，聊天仍可用。
- [默认 Bash 缺失时原本能进入 REPL] → 启动即报错；这是用户为保证环境上下文准确所选择的行为。
