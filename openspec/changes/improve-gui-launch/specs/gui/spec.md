# Spec Delta

## ADDED Requirements

### Requirement: 桌面入口无额外终端

系统 MUST 提供默认进入 GUI 的桌面启动入口。Windows 用户直接打开桌面应用时 MUST 不创建额外控制台；Linux 桌面启动入口 MUST 不要求终端。启动探测、工具执行与超时清理 MUST 不自动弹出 Windows 控制台。原有终端入口 MUST 继续支持 TUI 和 REPL。

#### Scenario: Windows 双击桌面应用

- **WHEN** 用户双击桌面应用并等待启动完成
- **THEN** 默认进入 GUI，启动期间与启动完成后均无额外控制台窗口

#### Scenario: Linux 桌面启动

- **WHEN** 用户通过已配置的桌面启动器打开应用
- **THEN** 直接显示 GUI，无需打开终端

#### Scenario: 后台工具与清理

- **WHEN** 用户在 GUI 中授权执行命令或命令超时触发清理
- **THEN** 不自动弹出 Windows 控制台，输出与失败仍在 GUI 中反馈

#### Scenario: 终端入口回归

- **WHEN** 用户使用原有终端构建启动交互或管道会话
- **THEN** 仍按原有配置进入 TUI 或 REPL

### Requirement: 桌面启动失败可见

桌面入口 MUST 在 GUI 中显示环境配置、界面模式与 Agent 初始化错误，不依赖可见终端。桌面构建不支持的终端模式 MUST 显示如何使用终端构建的提示。

#### Scenario: 配置解析或初始化失败

- **WHEN** 桌面应用读取的环境配置无效或缺少模型密钥
- **THEN** GUI 显示具体启动失败原因，用户可关闭窗口

#### Scenario: 桌面构建误选终端模式

- **WHEN** 桌面应用配置为终端界面
- **THEN** GUI 显示桌面构建只支持 GUI，并提示使用终端构建
