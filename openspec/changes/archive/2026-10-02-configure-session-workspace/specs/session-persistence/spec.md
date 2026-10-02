# Spec Delta

## MODIFIED Requirements

### Requirement: 列表与安全恢复

系统 MUST 提供 `/sessions` 显示当前 workspace 的近期会话，并提供 `/sessions --all` 显示所有 workspace 的近期存档和进程内会话；全部范围的条目 MUST 标注所属 workspace。系统 MUST 提供 `/open <session-id>` 打开进程内或已保存会话，并保留 `/resume <session-id>` 的兼容行为。恢复 MUST 验证模型、接口、服务端点、检查点格式、消息关联和会话记录的 workspace；成功后 MUST 切换至该 workspace、重新加载环境上下文、清空工具授权且不得自动重跑工具。打开当前会话 MUST 保持原状态，恢复失败 MUST 保持当前会话和授权。

#### Scenario: 恢复压缩会话

- **WHEN** 用户从另一 workspace 恢复曾经压缩并正常退出的会话
- **THEN** 系统切换到记录中的 workspace，Session ID、摘要与近期原文继续可用，后续输入沿用该会话

#### Scenario: 工具执行中断

- **WHEN** 会话在工具执行中断后恢复
- **THEN** 系统显示最近安全检查点及工具副作用未确认状态，并等待新的用户输入

#### Scenario: 不兼容或损坏的会话

- **WHEN** 目标会话的配置或格式不兼容，消息关联已损坏，或记录中的 workspace 已不存在、不是目录或不可访问
- **THEN** 系统拒绝恢复，说明原因并保留当前会话、workspace 和工具授权

#### Scenario: 打开当前会话

- **WHEN** 用户打开当前会话的 UUID
- **THEN** 当前历史、待保存内容、workspace 和授权均保持不变

#### Scenario: 当前目录列表

- **WHEN** 用户执行 `/sessions`
- **THEN** 系统合并显示当前 workspace 的进程内会话和近期存档，并按 UUID 去重

#### Scenario: 全部目录列表

- **WHEN** 用户执行 `/sessions --all` 或在 GUI 选择全部范围
- **THEN** 系统合并显示所有 workspace 的近期存档和进程内会话，并为每条记录显示所属目录

## ADDED Requirements

### Requirement: 会话独立保存 Workspace

系统 MUST 让每条进程内及持久化会话独立保存规范化的 workspace。关闭会话持久化时，进程内会话仍 MUST 在切换和往返打开时恢复自己的目录；保存失败时，原会话的 workspace 和待补写内容 MUST 留在该会话中供后续重试。

#### Scenario: 内存会话跨目录往返

- **WHEN** 会话持久化关闭，用户在 workspace A 对话、切换至 B，再打开 A 的进程内会话
- **THEN** 系统恢复 A 的目录和历史，B 的历史仍隔离保存

#### Scenario: 保存失败后切换

- **WHEN** workspace A 的会话保存失败后用户切换至 B
- **THEN** 系统允许切换，并在后续保存时继续补写 A 的原事件、检查点和 workspace

