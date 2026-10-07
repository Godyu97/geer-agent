# 模块索引

已知文件直接定位；未知归属搜索下表。先读模块 00 摘要，实现问题读 01 命中章节，相关事项才读 02。

| 模块 | 职责 | 代码入口 |
|---|---|---|
| [开发工具与测试隔离](dev-tooling/00-overview.md) | 管理构建/验证命令、受限测试服务、进程夹具和已安装仓库工作流。 | [Makefile](../../Makefile)、[scripts](../../scripts)、[tests/support](../../tests/support)、[.agents](../../.agents) |
| [构建入口](build-script/00-overview.md) | 按 Cargo feature 追踪 GUI/Web 产物，检查 Web 入口并调用可选 Tauri 构建。 | [build.rs](../../build.rs) |
| [程序入口与界面分派](entry/00-overview.md) | 从 main 进入 ui::run，加载配置并选择 REPL、TUI、GUI 或 Web。 | [src/main.rs](../../src/main.rs)、[src/ui/mod.rs](../../src/ui/mod.rs) |
| [Agent 编排](agent/00-overview.md) | 管理一次对话中的模型轮次、工具调用、预算、循环保护、记忆召回与会话保存。 | [src/agent](../../src/agent) |
| [配置与路径](configuration/00-overview.md) | 解析模型、UI、持久化、工具与资源配置，并集中处理 Bash/Windows 路径和后台子进程约定。 | [src/config](../../src/config) |
| [持久化适配](storage/00-overview.md) | 为会话、Trace 与长期记忆提供 SQL 和 MongoDB 存储接口及迁移。 | [src/dao](../../src/dao) |
| [界面中立交互契约](interaction/00-overview.md) | 定义 Agent 可提供的 Session 能力、命令解析和多界面共用的会话/记忆操作。 | [src/interaction](../../src/interaction) |
| [长期记忆](memory/00-overview.md) | 管理跨会话长期记忆的读写、精确去重、编辑删除、关键词搜索和主动召回。 | [src/memory](../../src/memory) |
| [Prompt 与上下文](prompt/00-overview.md) | 组装模型对话上下文、项目指令、会话快照和压缩所需数据。 | [src/prompt](../../src/prompt) |
| [模型协议适配](provider/00-overview.md) | 为 Agent 提供统一 ChatProvider 接口，适配 OpenAI Responses 与 Chat Completions 流式调用。 | [src/provider](../../src/provider) |
| [记忆检索排序](retrieval/00-overview.md) | 将长期记忆切成重叠片段，并对查询进行多语言词项提取和 BM25-lite 排序。 | [src/retrieval.rs](../../src/retrieval.rs) |
| [会话与工作区](session/00-overview.md) | 维护 workspace、会话运行状态、快照/事件恢复、历史列表和安全删除预览。 | [src/session](../../src/session) |
| [工具注册与执行](tools/00-overview.md) | 注册 Agent 可调用的文件、查询、Bash、Web 与记忆工具，并实施参数验证、授权和输出限制。 | [src/tools](../../src/tools) |
| [调用 Trace](trace/00-overview.md) | 采集模型、工具、用量和错误事件，并提供脱敏、状态与记录结构。 | [src/trace](../../src/trace) |
| [图形共用运行时](ui-app/00-overview.md) | 为 GUI/Web 共用 Agent 工作线程、命令队列、事件快照、授权 Gate 与关闭保存流程。 | [src/ui/app](../../src/ui/app) |
| [终端界面](ui-terminal/00-overview.md) | 提供按行交互的 REPL 与全屏 TUI，统一呈现消息、工具进度、会话和记忆操作。 | [src/ui/commands.rs](../../src/ui/commands.rs)、[src/ui/repl](../../src/ui/repl)、[src/ui/tui](../../src/ui/tui) |
| [桌面 GUI 宿主](ui-gui/00-overview.md) | 用 Tauri 窗口、IPC 命令与桌面 HostAdapter 连接共用 React 前端和 ui/app。 | [src/ui/gui](../../src/ui/gui) |
| [浏览器 Web 宿主](ui-web/00-overview.md) | 用 Axum 提供登录、静态资源、同源 HTTP/WebSocket 与共享 UI 事件传输。 | [src/ui/web](../../src/ui/web) |
| [共用 React 前端](frontend/00-overview.md) | 一套 React/TypeScript 应用用于桌面 GUI 和浏览器 Web，按构建模式注入不同宿主能力。 | [src/ui/frontend](../../src/ui/frontend) |

路径归属维护于 [catalog](../../.ai/catalog.json)；模块变化时同步本索引。三件套齐备不代表全部预读或业务已验收。
