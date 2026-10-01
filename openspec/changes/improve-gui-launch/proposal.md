# Proposal

## Why

桌面 GUI 沿用终端程序启动方式，Windows 双击时会额外打开控制台；后台系统与 Bash 探测也可能产生窗口闪烁。用户希望直接进入桌面界面，启动失败仍能得到可见反馈。

## What Changes

- 提供默认进入 GUI 的桌面构建与启动说明，Windows 桌面启动不创建额外终端，Linux 桌面启动器不要求终端。
- 启动探测、工具执行与超时清理不自动弹出 Windows 控制台。
- 桌面入口在界面创建前遇到配置错误时显示错误提示；已有终端构建继续支持 TUI/REPL。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `gui`：增加桌面无终端启动及可见失败反馈要求。

## Impact

影响构建入口、UI 模式选择、GUI 启动错误处理以及后台子进程创建参数。复用现有依赖与单一 Rust 二进制；总结写入 `doc/plan/`。

## Non-goals

不关闭用户已经打开的终端，不隐藏开发构建输出，不引入安装包、自动安装快捷方式、系统服务或新的 UI 框架；不更改授权、会话与模型协议。
