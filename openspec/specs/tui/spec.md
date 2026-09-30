# tui Specification

## Purpose

终端界面让用户通过键盘查看具有可读标题的历史会话，选择单条或多条记录并确认删除，在窄终端和错误情况下仍能理解当前操作，同时继续使用既有对话和授权流程。

## Requirements

### Requirement: 终端会话管理面板

TUI MUST 通过 F3、/sessions 或 /sessions --all 打开对应范围的会话面板。上下及翻页键 MUST 移动高亮，Space MUST 勾选，a MUST 全选或取消全选，Delete MUST 请求删除选中项，Enter MUST 打开高亮会话，Tab MUST 切换范围，Esc MUST 返回聊天。面板 MUST 显示标题、当前会话标记、选择数量和完整 UUID 查看位置，窄屏仍可操作。

#### Scenario: 多选与范围变化

- **WHEN** 用户勾选若干会话后按 Tab
- **THEN** 面板切换范围并清空选择，不把隐藏目标加入后续删除

#### Scenario: 返回聊天

- **WHEN** 用户按 Esc 关闭面板
- **THEN** 选择被清空，原聊天草稿与会话保持不变

### Requirement: 终端批量删除确认

TUI MUST 对选中目标显示一次默认取消的确认，展示标题、数量、完整 UUID、Trace 保留说明及当前会话替换影响。完成后 MUST 刷新列表并逐项展示结果，允许重试失败或清理告警；取消 MUST 不改变会话。工具授权与模型运行期间 MUST 不执行会话管理操作。

#### Scenario: 删除当前会话

- **WHEN** 用户确认删除包含当前会话的一组选中目标
- **THEN** 删除成功后旧消息和草稿被清空，当前状态指向原 workspace 下的新空会话，管理面板继续可用
