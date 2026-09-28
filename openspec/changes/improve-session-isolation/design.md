# Design

## Context

见 proposal.md。`Prompt` 保存 Chat/Responses 历史、摘要及待保存原始事件；`Agent` 目前另持一个 Session ID、一个上下文估算偏差和一个 `SessionRuntime`。`/reset` 清空这些状态，`/resume` 从数据库读快照。`SessionStore`/`TraceStore` 使用同一 SQL/MongoDB 底层表与迁移机制，默认都未启用。用户确认要默认共用 SQLite，并保留 UUID。

## Goals / Non-Goals

**Goals:** 让不同会话的完整运行状态一同移动；复用现有检查点和双协议消息格式；一个默认数据库服务会话和 Trace；保持故障可见及进程内历史不丢失。

**Non-Goals:** 跨进程自动合并冲突、自动继续最近会话、跨会话文件隔离或引入新的 Agent 框架。

## Decisions

### Rust 中的会话所有权

Day6 的“一个 Chat + 多个历史数组”在这里映射为 `Agent` 中一个共用 provider/工具执行器和一个 `SessionManager`。每个 `SessionState` 拥有 UUID、完整 `Prompt`、估算偏差及独立 `SessionRuntime`。`SessionManager` 保留一个活动状态与 `HashMap<String, SessionState>` 暂存状态。REPL 只传用户输入和命令，不再另持一个可能与当前 UUID 错配的 `Prompt`；工具循环仍借用当前 `Prompt`，不改变 provider 协议适配。Rust 所有权转移保证切换时摘要、原始事件和检查点队列不被单独遗落。只在 REPL 空闲的命令边界切换，不在流式输出或工具批次中间抢占。

当前会话准备好目标后才进行交换。内存目标直接从映射移出；数据库目标先在临时 `Prompt` 上验证再采用快照。失败时原状态和授权保持不变。真正交换后清空工具授权；打开活动 UUID 为无操作。`/new` 和 `/reset` 走同一新建路径。跨进程恢复重新生成 system 环境提示并继续使用版本 1 快照；进程内切换保留完整原文，而不是用脱敏快照覆盖其 `Prompt`。

不选每个会话一套 provider/工具对象：它们保存配置和可执行能力，既非对话历史，也会放大授权状态管理。暂不引入会话名称和新存储字段，因此旧 UUID/快照可原样恢复。

### 保存、故障与异步边界

复用现有“追加事件后以 revision 发布检查点”的 DAO 契约；`SessionRuntime` 的 revision、事件头、待写队列和中断标记属于各自 `SessionState`。保存返回能区分成功、待补写与 revision 冲突的状态，警告不带连接串。写入结果不确定时保留原有读回确认。`/save`、退出和 EOF 顺序重试所有待写会话；未保存 UUID 明确显示。冲突不自动改本地 revision 或覆盖数据库，内存会话可继续。启动连接失败时状态仍可进程内切换，退出仅承诺最后已发布的检查点。

`Agent` 的异步模型步骤仍只借用当前会话的消息快照；等待期间没有 REPL 命令进入。工具调用完成后照旧保存安全检查点，失败不重放工具。Trace 的 request ID 每个模型步骤生成，Session ID 从活动状态获取。工具授权在所有真实切换后清空；失败打开不碰授权。

### 配置与单次连接

配置解析保持在 `config`：只读取公共 `GEER_AGENT_DATABASE`/`GEER_AGENT_DATABASE_URL`，未设置时回退默认 `sqlite://.db/geer.sqlite?mode=rwc`。旧的 Trace/Session 专用数据库变量不再读取。两个 `on|off` 开关在确定有效数据库后分别决定是否启用功能；显式无效配置即使功能被关闭也报错，避免下次打开时才发现拼写错误。SQLite 相对路径以启动工作目录为准，默认文件的父目录在连接前创建。配置出错返回 `Result`，连接或写入出错告警且保留内存状态。

两个功能同时启用时，只调用一次现有 DAO 连接/迁移入口，再克隆共享的底层 SQL 连接或 MongoDB collection 句柄供两个接口使用。默认启用 Trace 后，脱敏同时考虑 API key、公共数据库 URL 和认证字段。无需新增 crate：`HashMap`、路径和目录使用 std；现有 SeaORM/MongoDB 连接句柄已经可共享。不另建 JSON 文件或新表，以免和已有检查点产生两套真相。

## Risks / Trade-offs

- [默认本地文件保存用户对话和完整 Trace] → 文件置于 Git 忽略目录；文档明确其内容及关闭开关。
- [多个进程写同一会话] → 延续 revision 条件更新，冲突显示并保留进程内数据；不在本轮自动合并。
- [数据库不可用] → 告警后保留进程内切换，明确只有已成功发布的记录能跨进程恢复。
- [默认持久化影响测试] → 子进程测试使用临时工作目录或显式关闭存储，避免写到项目数据库。

## Migration Plan

先增量加入会话状态和命令，再切换配置默认值并复用连接；保留原表、集合、迁移标识和版本 1 快照。需要继续使用已有数据库时，通过公共数据库变量指定其地址。回滚至旧程序时新默认目录可以留在 Git 忽略范围内，但旧程序需显式配置数据库才能继续读取。
