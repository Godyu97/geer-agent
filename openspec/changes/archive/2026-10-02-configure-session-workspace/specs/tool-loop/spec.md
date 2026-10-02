# Spec Delta

## ADDED Requirements

### Requirement: 工具使用活动 Workspace

系统 MUST 让 Bash、文件及文件查询工具统一使用活动会话的 workspace：Bash MUST 在该目录启动，相对文件路径和省略路径的查询 MUST 以该目录解析。切换或恢复到另一 workspace 后，后续工具调用 MUST 立即使用新目录，并 MUST 重新请求会话级工具授权。绝对路径与 `~/` 的既有语义 MUST 保持有效。

#### Scenario: 相对工具路径随目录切换

- **WHEN** 两个 workspace 包含同名但内容不同的文件，用户切换后请求读取相对路径
- **THEN** 工具返回新 workspace 中的文件内容，不再读取启动目录或旧 workspace

#### Scenario: Bash 工作目录随会话恢复

- **WHEN** 用户打开属于另一 workspace 的旧会话并运行打印工作目录的 Bash 命令
- **THEN** Bash 返回恢复会话的 workspace，并在执行前重新请求 Bash 授权

#### Scenario: 查询省略路径

- **WHEN** 模型在活动 workspace 中调用不带 path 的目录或搜索工具
- **THEN** 查询以活动 workspace 为根执行

