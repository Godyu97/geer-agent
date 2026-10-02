# Proposal

## Why

对应 GeekAgent Day10（能力 `project-memory`）：现有会话可以保存，但新会话还不能自动遵守项目规则或找回跨会话经验。增加项目指令和数据库长期记忆，让用户能跨会话使用偏好、事实与决定，也能直接整理这些记忆。

## What Changes

- 只加载当前 workspace 根目录的 AGENTS.md，下一条消息使用修改后的全文；切换或恢复会话后使用对应项目的指令。
- 长期记忆按数据库全局共享，跨会话、workspace 与重启保留，独立于会话和 Trace。
- Agent 可自动调用 memory_write 保存重要信息、memory_search 按关键词搜索；不自动把全部记忆带给模型。
- REPL、TUI、GUI、Web 提供查看、搜索、添加、编辑、单条删除和清空；删除与全局清空默认取消，UI 显示指令加载状态、记忆数量及真实故障。

## Capabilities

### New Capabilities

- `project-memory`：workspace 根项目指令、全局数据库长期记忆、模型记忆工具和四种界面的记忆管理。

### Modified Capabilities

无。工具循环、会话保存和既有会话管理契约保持有效；新增行为由 project-memory 描述。

## Impact

增量扩展 prompt、session、dao、config、agent、tools、interaction 与四种 UI；SQL 增加单独记忆表，MongoDB 增加同名集合。扩展图形快照和确认事件，补充测试夹具、README 与架构说明，不新增依赖。

## Non-goals

不合并多级项目指令，不引入 embedding、RAG、自动检索注入、插件平台或云同步。清空长期记忆不删除会话、Trace，也不清除聊天历史中已出现的记忆内容。
