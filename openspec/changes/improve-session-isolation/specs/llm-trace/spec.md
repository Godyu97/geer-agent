# Spec Delta

## MODIFIED Requirements

### Requirement: 会话与调用标识

系统 MUST 在每次启动及 `/new`、`/reset` 后产生并显示新的 Session ID；打开旧会话后 MUST 沿用其 Session ID。同一会话内的模型步骤 MUST 各有独立 Request ID，同一步骤的重试 MUST 共用该 Request ID，记录 MUST 关联实际活动会话。

#### Scenario: 重置会话

- **WHEN** 用户在一次启动内发送多条消息，然后输入 `/reset` 再发送消息
- **THEN** 重置前的调用具有相同 Session ID，重置后的 Session ID 与之前不同，每个模型步骤都有独立 Request ID

#### Scenario: 模型步骤重试

- **WHEN** 一个模型步骤在获得有效响应前重试
- **THEN** 该步骤只有一个 Request ID，记录其总尝试次数

#### Scenario: 切换会话

- **WHEN** 用户从 A 切到 B 后调用模型，再切回 A 调用模型
- **THEN** 三段调用分别关联 A、B、A 的 Session ID

### Requirement: 可选的调用记录

系统 MUST 在未配置数据库时默认将模型调用记录保存到当前工作目录的 SQLite 数据库，并允许用户单独关闭 Trace。系统 MUST 只通过公共数据库变量选择数据库，不再读取旧的 Trace/会话专用数据库变量；无效的显式公共数据库配置 MUST 在启动时明确报错。Trace 与会话同时启用时 MUST 写入同一数据库。

#### Scenario: 未选择数据库

- **WHEN** 用户没有设置数据库或 Trace 开关
- **THEN** REPL 正常启动，模型步骤默认写入当前工作目录的 SQLite 数据库

#### Scenario: 无效配置

- **WHEN** 用户选择不支持的数据库，或者选择非 SQLite 数据库但没有提供连接地址
- **THEN** 程序在进入交互前显示明确的配置错误

#### Scenario: 分别关闭

- **WHEN** 用户只关闭 Trace 或只关闭会话持久化
- **THEN** 未关闭的记录功能继续运行，已关闭的记录功能不写入数据

#### Scenario: 公共数据库配置

- **WHEN** 用户设置公共数据库类型和连接地址并同时启用 Trace 与会话持久化
- **THEN** 两种记录写入该数据库，旧的专用数据库变量不改变目标数据库
