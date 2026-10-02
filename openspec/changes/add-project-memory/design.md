# Design

## Context

动机见 proposal。现有 Prompt 将 system 与可持久化历史分开；SessionManager 管理活动和停放会话，切换 workspace 会重建提示。DAO 已支持 SQLite/PostgreSQL/MySQL/MongoDB；四种 UI 共用 interaction，GUI/Web 共用串行工作线程与完整状态缓存。沿这些边界增量扩展。

## Goals / Non-Goals

保证项目规则跟随 workspace，记忆数据不依赖会话保存且不会被会话切换覆盖。只扩展现有模块，不引入检索框架、第二套前端或新的授权平台。

## Decisions

### 指令与提示所有权

PromptContext 保留环境信息；增加根 AGENTS.md 加载与指令文本组装，Prompt 提供替换 system 的入口。SessionState 保存本轮指令加载状态；读取成功后才替换提示或提交 workspace/open，错误保留原状态。每条用户消息开始前读取一次，工具续轮复用这份文本；新建同 workspace 会话沿用当前指令，到下一条消息重新读取。标准库同步读取即可，不需要文件监控或新的 tokio feature。全文只放入 Chat system / Responses instructions，快照和压缩仍只处理历史；上下文估算自然计入 system 大小。

不选全局启动目录缓存，因为会话能够切换 workspace；不每个工具轮重读，避免一次请求执行中规则变化。

### 数据与记忆服务

新增 memory 模块，MemoryEntry 为拥有所有权的 UUID、content、created_at_ms、updated_at_ms。SQL 新迁移仅新增 agent_memories 表，MongoDB 使用同名集合；没有 workspace/session 外键。DAO 暴露可克隆 MemoryStore，并与会话、Trace 共用连接。每次写入只处理目标记录，清空只删除记忆记录，绝不通过会话 flush 保存一份旧记忆列表。

MemoryService 由 Agent 与 Tools 共享单线程 Rc；数据库操作异步执行，借用不跨 await 保持 RefCell 可变引用。统一校验、trim、精确去重、更新与排序，数据库成功后更新状态；查询直接读库，失败返回 Result，不伪装为空结果。UI 状态可以缓存最近成功数量，但错误必须标记不可用。搜索在 Rust 按 Unicode 小写后的子串匹配，保证四种后端不受 SQL collation 差异影响；命中数降序，同分创建时间/UUID 升序，最多十条。

教程的 JSON 字符串数组改为现有 DB 的独立记录，UUID 使 UI 能稳定编辑/删除。不加 embedding 或新的 crate，现有 std、Serde、UUID 和数据库驱动足够。

### 配置、工具与流式输出

增加 GEER_AGENT_MEMORY=on|off（默认 on），复用 GEER_AGENT_DATABASE/URL，独立于 Trace 和会话开关。初始化失败只关闭记忆，继续聊天并给出诊断。工具注册增加 memory_write(content)、memory_search(query)，都按用户选择自动授权、串行执行；受 GEER_AGENT_TOOLS 和记忆可用性过滤。输出沿用 ToolOutput 与预算限制，截断提示只涉及记忆查询。系统提示说明保存有长期价值的信息、不保存临时任务进度或随时可从文件读取的内容，并在回忆时先搜索。流式输出及模型循环保持现有结构。

### 共用管理与图形协议

interaction 增加 MemoryCommand、结构化结果与确认预览，Session 暴露列表和变更能力。共用命令为 /memory、search、add、edit <uuid>、delete [--yes] <uuid>、clear [--yes]，正文取剩余文本，不执行 Shell 展开。删除目标不存在返回已不存在，编辑不存在返回错误，空库清空返回零条。

REPL 用默认取消确认，非交互不消费下一行；TUI F4 或 /memory 进入面板，上下键选择，Enter 全文，/ 搜索，a 添加，e 编辑，Delete 删除，c 清空，Esc 返回。编辑/搜索态 Enter 提交、Esc 取消；单独编辑草稿，不动聊天输入。

SessionStatus 增加 instructions_loaded 与记忆状态；AppSnapshot 增加全局列表。GUI/Web 共用 MemoryPanel 弹窗和本地关键词筛选（与 Rust 使用同一排序规则）；后端操作完成刷新快照。扩展 EventHub 的预览为带类型的会话/记忆删除/全局清空预览，绑定 client、revision 与确切目标；memory --yes 在两种图形宿主都必须匹配预览，终端显式 --yes 可直接执行。旧预览在任何后续操作或断线时失效。记忆数据仅给 UI，不加入 transcript 或模型 system。

## Risks / Trade-offs

- 全局清空影响所有 workspace → 确认显示全局范围与总数，保留会话/Trace，过期版本不能执行。
- 已进入聊天历史的记忆仍可被模型看见 → UI 说明清空长期存储不会擦除历史。
- 字符串关键词可能漏召回、完整列表随记忆增长变大 → 此学习切片接受，最多十条模型结果且输出有界；不提前扩展 RAG。
- 新默认开关可能使旧测试写入程序旁 DB → 所有进程夹具明确关闭记忆或指定本轮私有 DB；指令测试使用临时 workspace，外部后端只用专用测试库。
- 多进程同时编辑不做协作合并 → 本次复用单进程串行及 Web 状态版本，编辑旧目标报错，不从缓存重建已删除记录。

## Migration Plan

追加 SQL 迁移，不修改已有表和会话快照版本；旧会话加载当前根指令。MongoDB 新集合按需初始化。部署沿既有启动迁移；关闭 GEER_AGENT_MEMORY 可停止记忆能力，数据保留，不自动执行删除迁移。
