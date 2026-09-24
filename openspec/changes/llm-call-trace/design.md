# Design

## Context

见 proposal.md - Why。当前 `repl::Session::reset` 清除工具授权，`Prompt::reset` 清除历史；`AgentRuntime` 每条用户输入生成一个 `agentRunId`，两种模型接口由 `ChatProvider::complete_step` 汇合。Responses 在单个步骤内自行重试，Chat Completions 使用 SDK 的默认重试。现有工具摘要只写 stderr 且不含参数或结果原文；Trace 是独立、显式启用的数据面，不改变这类摘要。

## Goals / Non-Goals

**Goals:** 一个模型步骤一条记录；四种数据库的读写语义一致；存储故障不改变模型调用结果。

**Non-Goals:** 不保存每个 SSE 事件、不新增终端查询命令、不让数据库承载会话历史或自动过期策略。

## Decisions

- **所有权与标识。** `Agent` 拥有 UUID v4 `session_id` 和可选的 DAO；构造时生成 Session ID，`reset` 更新它，REPL 通过 `Session::session_id()` 显示。`AgentRuntime` 的 `run_id` 仍按用户输入生成。每次进入模型步骤前生成 UUID v4 `request_id`，包括无工具的最终回答步骤；步骤内 HTTP 重试不重新生成 ID。
- **模块与接口。** `trace` 只定义 `TraceRecord`、`TraceCapture`、`TraceWriter`、`TraceReader`、分页游标和错误；`dao` 依赖 `trace` 并实现后端选择。两个异步 trait 提供 `write_one` / `write_batch`、`get_one` / `get_batch` / `list_session_page`。批量读取与传入 ID 一一对应，缺失值保留原位置；分页按 `(started_at_ms, request_id)` 升序，单页上限 100。运行时只选择一个后端，由 DAO 枚举静态分派，不引入 trait-object 所需的额外宏。
- **采集时机。** `Agent` 在模型步骤外层计时并在调用完成、失败或被预算超时取消后写一条 Trace；Trace 写入置于模型超时 future 之外，避免超时丢掉已收集信息。`provider` 在构造完实际请求体后把它序列化到 `TraceCapture`，收集流式正文及工具调用片段，成功时替换为完整合并响应；失败时保留片段。仅保存请求体，不读取 HTTP 头。Responses 的显式重试和 Chat 的传输层计数器都汇总为总尝试次数，后者保留 SDK 原有重试策略。
- **记录形状。** `request_id` 是不可变主键；记录还包含 `session_id`、`agent_run_id`、可得的上游响应 ID、API、模型、UTC 毫秒时间、单调计时得到的耗时、尝试次数、状态、可得的 Token 用量、请求/响应 JSON 和错误。SQL 用同一实体与 JSON 列，MongoDB 用同字段文档及 `_id=request_id`；四种后端都建立 Session/时间/Request 索引。记录失败时过滤已知 API key；数据库错误告警不打印连接串。
- **批量语义。** SQL 使用批量 INSERT，MongoDB 使用无序 `insert_many`。批次成功即逐项成功；报错后按 Request ID 回读，必要时单条补写未存项，再回读核对。相同 ID 且内容相同视为幂等成功，内容不同为冲突；若连接故障使结果无法核实，返回批次级错误。空批次成功且无结果。读取使用后端原生的集合查询。
- **配置和初始化。** 沿用现有 `.env` 与进程环境变量优先级。`GEER_AGENT_TRACE_DATABASE` 为空即关闭；非空仅接受四种后端，且必须提供匹配协议的 `GEER_AGENT_TRACE_DATABASE_URL`；Mongo URI 必须指定数据库名。格式错误在启动时失败；连接或初始化错误告警后关闭本次运行的 Trace，后续写入错误告警后下一次仍可重试。SQL 在启动时运行本变更的版本化迁移；MongoDB 验证连接并建立索引。一个构建包含四种驱动，运行时无需重编译即可切换。
- **依赖选择。** `sea-orm 2.0.3` 覆盖三个 SQL 后端及异步查询，`sea-orm-migration 2.0.3` 维护版本化表结构；`mongodb 3.9.1` 是 MongoDB 官方异步驱动。标准库不提供这些数据库协议、连接池或迁移。`serde` 用于同一记录在 JSON/BSON 间转换；`uuid` 提供随机唯一标识；`tower` 仅用于在不改变 Chat 重试策略时统计传输尝试次数。已有 `tokio`、`serde_json`、`chrono` 继续使用。不选 SQLx，因为它不是 ORM；不选 Diesel 加异步包装，因为本仓库已经是 Tokio 流式入口，SeaORM 能共用一套 SQL 实体与查询。

## Risks / Trade-offs

- [完整请求可能包含用户对话和工具结果] → 默认关闭持久化；文档说明启用后须保护数据库与备份，保持现有工具 stderr 摘要的无正文约束。
- [数据库批量写入可能部分成功] → 回读核对和逐项结果；无法核对时报告未知批次错误，不宣称整批成功。
- [连接或迁移耗时影响启动，写入耗时影响下一步] → 设置有限连接/服务器选择超时；MongoDB 操作等待驱动返回，不直接丢弃其 future。存储错误不转成模型错误。
- [四种驱动增加编译体积] → 用户要求同一 `.env` 可切换四种后端；通过定向测试与 Cargo 锁文件控制兼容性。

## Migration Plan

旧配置不启用 Trace，无既有数据需要转换。启用时 SQL 自动创建 `llm_traces` 表及索引，MongoDB 创建集合所需索引；回退只需移除 Trace 配置，已存记录留在用户数据库中。示例 SQLite 路径及 WAL 辅助文件加入 Git 忽略。
