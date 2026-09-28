# Proposal

## Why

当前 REPL 只有一条活动对话：`/reset` 新建 ID 后，旧对话只能从数据库恢复；数据库未启用或一次保存失败时，切换可能丢掉进程内尚未保存的内容。Day6 对应能力是让不同话题在同一进程内各自保有上下文，并能在退出后继续。

## What Changes

- 增加 `/new`、`/open <session-id>`、`/save`；`/reset` 新建但保留旧会话，`/resume` 兼容打开命令。`/sessions` 合并进程内和已保存会话，标明当前及保存状态。
- 会话切换保留各自历史、摘要和未保存内容；真正切换时清空工具授权。失败打开不改变当前会话。
- **BREAKING**：未配置数据库时，会话和模型调用 Trace 默认写入工作目录的 `./.db/geer.sqlite`；仅使用公共数据库配置，并提供两个独立关闭开关。旧的专用数据库变量不再生效。
- 保存失败或 revision 冲突时保留进程内内容，退出前尝试补写，并显示仍未保存的会话 ID。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `session-persistence`：增量加入进程内多会话、默认保存、列表与失败保留行为。
- `llm-trace`：默认开启并共用数据库，同时允许独立关闭；调用始终关联活动会话。

## Impact

影响 REPL 命令、Agent 会话状态、数据库配置与连接初始化、帮助和使用文档；保留已有表和快照格式，不增加依赖。

## Non-goals

不增加会话命名、分叉、删除、文件系统隔离、后台并发调度或 JSON 导入导出。
