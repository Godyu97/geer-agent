# Proposal

## Why

历史会话当前主要显示 UUID，难以辨认，也缺少删除入口。需要在现有 Day6 会话管理与 Day7 TUI 能力上补充可运行的整理功能，让用户能辨认并批量清理不需要的历史。

## What Changes

- 会话列表与状态显示首条用户消息前 20 个字符作为标题，保留 UUID 唯一标识；标题不单独持久化。
- 支持按完整 UUID 单条或批量删除快照与消息事件，保留 Trace，逐项反馈结果并支持重试。
- 删除当前会话成功后，在原 workspace 新建空会话并清空授权；失败保留原状态。
- GUI 增加管理模式、勾选和全选；TUI 增加会话管理面板和键盘多选。
- 提供 /delete 命令及一次确认；管道输入需显式 --yes。

## Capabilities

### New Capabilities

- `gui`：复用 add-desktop-gui 中尚未归档的能力路径，增加历史会话多选管理。
- `tui`：复用 add-tui-usage-panel 中尚未归档的能力路径，增加历史会话管理面板。

### Modified Capabilities

- `session-persistence`：增加派生展示标题、批量删除及保存后不复活的行为。

## Non-goals

不增加分页、回收站、手工命名或搜索；不删除 Trace，不变更 UUID、数据库结构或快照格式，不重写现有 Agent 路径。

## Impact

影响 prompt 的内存展示数据、session/DAO、interaction 命令、REPL、GUI/TUI 与对应测试。复用现有依赖和串行执行机制；不新增配置与依赖。
