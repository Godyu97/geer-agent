# workspace-management Specification

## Purpose

让用户在 Agent 运行期间选择项目目录，并保证会话、模型环境、工具执行和各界面展示始终使用同一个经过校验的 workspace，同时在切换失败时完整保留原状态。

## Requirements

### Requirement: Workspace 路径解析与切换

系统 MUST 以启动目录作为首个会话的 workspace，并 MUST 允许用户查看当前绝对目录或设置新目录。输入 MUST 支持绝对路径、相对当前 workspace 的路径、`~/` 和一层配对引号；系统 MUST 将有效输入规范化为已存在目录的绝对路径，不得执行环境变量、通配符或其他 Shell 展开。成功切换 MUST 保存并保留原会话，在目标目录创建新的 UUID 会话；设置规范化后相同的目录 MUST 保持当前会话不变。

#### Scenario: 切换到相对目录

- **WHEN** 用户设置一个相对当前 workspace 且实际存在的目录
- **THEN** 系统显示规范化的绝对目录和新的 Session ID，原会话仍可打开

#### Scenario: 带空格或中文的路径

- **WHEN** 用户提交带空格、中文或一层配对引号的有效目录
- **THEN** 系统去除外层引号后按原路径切换，不把路径交给 Shell 解释

#### Scenario: 无效目录保持状态

- **WHEN** 用户提交不存在路径、普通文件、不可访问目录或不配对引号
- **THEN** 系统说明错误，并保持当前会话、workspace、对话历史和工具授权不变

#### Scenario: 设置当前目录

- **WHEN** 用户提交的路径规范化后等于当前 workspace
- **THEN** 系统报告当前目录未变，不新建会话且不清空授权

### Requirement: 公共命令入口

系统 MUST 提供 `/workspace` 显示当前完整路径，并提供 `/workspace <path>` 执行 workspace 切换；`/new` 与 `/reset` MUST 在当前 workspace 创建新会话。文本 REPL MUST 在启动和成功切换后显示 workspace 与 Session ID。

#### Scenario: 查看当前 Workspace

- **WHEN** 用户输入不带参数的 `/workspace`
- **THEN** 系统显示当前会话的规范化绝对目录且不修改状态

#### Scenario: 新会话继承目录

- **WHEN** 用户切换 workspace 后执行 `/new` 或 `/reset`
- **THEN** 新会话继续使用已切换的目录

### Requirement: TUI Workspace 入口

TUI MUST 显示当前 workspace，并 MUST 支持公共命令及 F2 路径编辑入口。F2 编辑 MUST 预填当前路径；Enter MUST 提交，Esc MUST 取消。打开编辑入口或发生校验错误 MUST 保留原聊天草稿，错误后 MUST 保留待修正的路径文本；窄屏下仍 MUST 提供可发现的快捷提示。

#### Scenario: 快捷键切换

- **WHEN** 用户按 F2、修改预填路径并按 Enter
- **THEN** TUI 通过公共切换语义更新 workspace、Session ID 和状态区

#### Scenario: 取消目录编辑

- **WHEN** 用户在 workspace 编辑状态按 Esc
- **THEN** TUI 返回聊天输入，保留原聊天草稿且不改变会话

#### Scenario: 路径校验失败

- **WHEN** 用户在 workspace 编辑状态提交无效目录
- **THEN** TUI 显示错误、保留路径文本供修改，并保持原会话和聊天草稿

### Requirement: GUI Workspace 入口

GUI MUST 在侧栏显示当前 workspace，并 MUST 提供手工路径输入与单目录系统选择器。目录选择取消 MUST 不改变输入和会话；选择成功 MUST 只填充输入，用户确认后才切换。切换期间 MUST 沿用现有串行和忙碌限制，错误 MUST 显示在设置区域且不清除输入。

#### Scenario: 手工输入切换

- **WHEN** 用户输入有效目录并选择“切换并新建会话”
- **THEN** GUI 在请求完成后显示新的 workspace、Session ID 和对应空白对话

#### Scenario: 浏览后取消

- **WHEN** 用户打开系统目录选择器后取消
- **THEN** GUI 保持原路径输入、会话和 workspace

#### Scenario: 忙碌期间设置

- **WHEN** 模型请求、工具授权或关闭流程仍在进行
- **THEN** GUI 禁用 workspace 切换入口，避免与当前轮次并发
