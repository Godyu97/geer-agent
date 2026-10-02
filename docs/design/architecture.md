# geer-agent 技术架构与优化方案

本文梳理当前项目的模块职责、运行流程和依赖边界，并记录 REPL 与 UI 分层重构。现在文本界面位于 `ui/repl`，只负责输入、渲染与交互确认；四种 UI 共用 `interaction::execute` 执行消息和会话命令。Agent 继续实现会话能力与模型/工具编排，Provider 负责协议交互。

原顶层 REPL 没有自己的模型循环，但仍直接分派新建、保存、压缩、打开、workspace 和删除等 Session 操作；TUI/GUI 各自重复这套逻辑。本次抽取的是这些共用操作，并将终端授权输入迁入 UI。正文、工具进度、重试和压缩通知仍使用现有字符串回调，结构化模型事件列为后续优化。

## 分析范围与依据

- 初版分析：2026 年 9 月 30 日；GUI/Web 接入更新：2026 年 10 月 1 日。
- 依据：当前工作区源码、依赖清单、已有测试与相关 OpenSpec 设计；本次重构从 HEAD `8482deb` 开始。
- 已实施范围：[centralize-ui-session-commands](/home/lihongyu/projects/geer-agent/openspec/changes/centralize-ui-session-commands/design.md)，使用 `skip_specs`，沿用当前行为契约。
- 验证：共用执行单元测试、三种界面相关回归与受限构建检查，具体记录见文末。

项目已经具备工具循环、文件工具、历史压缩、会话持久化、TUI 与可选 GUI。本次同步了治理与源码导读；历史 OpenSpec 设计仍描述各自阶段，以现行源码与行为契约核对现状。架构优化继续遵循增量开发和测试隔离约束。

## 技术栈与运行入口

### 技术栈

| 范围 | 当前选择 | 在项目中的作用 |
| --- | --- | --- |
| 程序形态 | Rust 2024 edition，单一二进制 crate | 模块在同一可执行文件内组合 |
| 异步执行 | Tokio current-thread runtime | 驱动模型流、数据库与子进程；文件操作另有阻塞任务 |
| 模型接口 | async-openai，Responses 与 Chat Completions | 访问用户配置的 OpenAI 兼容端点，解析流式正文、工具调用和用量 |
| 文本界面 | 标准输入输出与 ANSI 颜色 | 按行输入、命令处理、流式输出和管道兼容 |
| 全屏终端 | Ratatui 与 Crossterm | 键盘事件、布局、颜色、滚动和终端恢复 |
| 桌面界面 | 可选 Tauri 2 | 窗口、IPC、目录选择和剪贴板 |
| 浏览器界面 | 可选 Axum 0.8、include_dir、WebSocket | HTTP 监听、口令与凭证、同源校验、资源与事件传输 |
| 共用图形前端 | React/TypeScript、Vite/tsc/Vitest，Bun 1.4.2 管理 | 一套交互、状态、协议、Mocha 样式，构建时选择宿主 |
| 数据持久化 | SeaORM 与 MongoDB driver | SQLite、PostgreSQL、MySQL、MongoDB 的会话、Trace 与长期记忆适配 |
| 配置与数据 | dotenvy、Serde、serde_json、UUID | 环境配置、快照、事件与标识符 |

依赖依据见 [Cargo.toml](/home/lihongyu/projects/geer-agent/Cargo.toml) 与 [共用前端依赖清单](/home/lihongyu/projects/geer-agent/src/ui/frontend/package.json)。GUI 依赖由 `gui` feature 控制；`embed-env` 在编译时内嵌配置。数据库驱动目前属于默认 Rust 依赖，运行时关闭持久化不会移除它们的编译成本。

### 启动流程

[main](/home/lihongyu/projects/geer-agent/src/main.rs:16) 调用 [ui::run](/home/lihongyu/projects/geer-agent/src/ui/mod.rs:32)，后者加载环境配置并解析 `GEER_AGENT_UI`：

| 配置 | 运行路径 |
| --- | --- |
| `auto`，stdin 与 stdout 均为终端 | TUI |
| `auto`，任一端接管道 | 文本 REPL |
| `repl` | 文本 REPL |
| `tui` | 要求 stdin 与 stdout 均为终端，否则报错 |
| `gui` | 要求编译时启用 `gui` feature，否则提示构建方式 |
| `web` | 要求 `web` feature，监听 0.0.0.0:8827（端口可 env 配置） |

终端路径在 `ui::run` 中创建 Tokio runtime 和 Agent，再调用具体界面；GUI 路径进入 Tauri，Web 路径创建 HTTP 服务；两者在 `ui/app` 独立工作线程内创建 runtime 和 Agent。

这里的 `ui::run` 同时承担启动组合职责：它决定用哪个界面，并连接会话对象与界面回调。这样的组合点可以引用 `agent`；业务模块仍不应引用具体 UI。

## GUI / Web 接入与 React TS 复用（2026-10-01）

本次增量见 [add-web-ui 设计](/home/lihongyu/projects/geer-agent/openspec/changes/add-web-ui/design.md)。终端继续借用 `interaction::Session`；两种图形宿主共用 `ui/app`，所有会话写操作仍交给 `interaction::execute`，没有第二个 Agent 或数据库模型。

```mermaid
flowchart LR
    React["共用 React App / reducer / protocol / CSS"] --> Desktop["HostAdapter: desktop"]
    React --> Browser["HostAdapter: web"]
    Desktop --> IPC["ui/gui: Tauri IPC / 窗口"]
    Browser --> HTTP["ui/web: Cookie / Origin / HTTP / WS"]
    IPC --> Runtime["ui/app: 串行命令 / 工作线程"]
    HTTP --> Runtime
    Runtime --> Execute["interaction::execute → Agent"]
    Runtime --> Hub["完整快照 + 当前流 + 授权 + 20 条诊断"]
    Hub --> IPC
    Hub --> HTTP
    HTTP --> Gate["独立授权应答 / 首答生效"]
    IPC --> Gate
    Gate --> Runtime
```

Agent 含非 Send 确认回调，只在工作线程创建和使用。UI/网络跨线程发送拥有所有权的字符串和快照。授权回复使用独立 Gate，能唤醒正在等待的模型/工具操作；不会排在同一工作队列后面。

事件中心在同一锁内注册客户端并发送缓存完整状态，连接恢复无需等待被模型或授权占用的工作线程。每个操作有全局数字编号、UUID 和来源客户端，快照有递增 revision。忙碌提交立即拒绝，旧 revision 不能把消息写入另一端刚切换的会话。删除预览只发给来源端，Web 确认必须匹配原 UUID 集与修订号，任何后续操作使旧预览失效；GUI 的既有显式 --yes 命令保持兼容。

Web 是单用户多浏览器：所有端共用当前会话/workspace，第一份有效授权回复生效；120 秒或全部端断线默认拒绝。每端 256 项有界队列，最多 16 个连接；慢端关闭后通过完整状态重连。输入限制 65536 字节，WS 消息/帧限制 512 KiB，Ping/Pong 15/45 秒。`/exit` 共用保存判定后只退出来源页面；Ctrl+C/SIGTERM 停接新命令、取消授权并等待当前操作和保存，失败非零退出。

Make 的 build/release/run 统一启用 gui,web，并先构建两种静态前端；通用程序运行时由 GEER_AGENT_UI 选择界面。Windows 的 build/release 额外启用 desktop-gui 在独立 target/desktop-gui 缓存目录编译，成功后将桌面程序复制为对应 debug/release 目录的 geer-agent-desktop.exe；通用 geer-agent.exe 保留控制台与四种界面，桌面程序默认进入 GUI 且无额外控制台。设置 CARGO_TARGET_DIR 时两类产物与桌面缓存跟随该根目录。Linux 构建和 make run 仍只生成通用程序。直接 Cargo 默认仍是终端构建，不新增 Rust 入口或运行期控制台管理。

WebConfig 仅在选中 Web 时解析：固定 IPv4 全接口、默认 8827，`GEER_AGENT_WEB_PORT` 可改。`GEER_AGENT_WEB_TOKEN` 非空时固定，否则生成随机口令并只打印一次。登录换取内存随机凭证，Cookie 为 HttpOnly/SameSite=Strict；重启失效，POST 和 WS 核对 Origin/Host。模型密钥不传给浏览器。静态页面由 include_dir 嵌入二进制，可独立分发。

**前端管理建议已落地：** [src/ui/frontend](/home/lihongyu/projects/geer-agent/src/ui/frontend/package.json) 是唯一包，只提交 bun.lock，Bun 冻结安装和 bun run 统一脚本。Vite 模式把 `@host` 编译为桌面或浏览器适配器，输出 dist/gui 与 dist/web；浏览器包没有 Tauri IPC。协议在 protocol.ts，纯 reducer 在 model.ts，App 与 CSS 共用；WebGate 只负责登录，宿主适配器只负责连接、提交、授权、复制和关闭。后续按交互能力拆组件即可，不建立两个 React 工程或提前抽发布库。草稿按会话 UUID 保留；375px 布局使用会话/状态抽屉，HTTP 剪贴板失败提供手动复制。

## 项目指令与长期记忆（2026-10-02）

[add-project-memory 设计](../../openspec/changes/add-project-memory/design.md) 在现有 Prompt、Session、DAO 和公共命令路径上增加能力，无新增 crate。`ProjectInstructions` 只读取 workspace 根 `AGENTS.md`；SessionManager 在切换或恢复前读取目标规则，成功后替换独立系统提示。Agent 每条用户消息前刷新一次，同一工具循环共享快照；规则计入上下文估算，不写入会话快照或压缩输入。

`memory::MemoryService` 由 Agent 和 Tools 共享单线程 Rc，提供列表、关键词搜索、去重添加、编辑、单条删除及清空。记录在 `agent_memories` 独立表/集合中，不含 workspace/session 外键；SQL 用新增版本化迁移，MongoDB 按需建立集合。三个持久化开关独立，同配置复用连接。操作直接读取或写入数据库，只缓存状态和最近成功数量，失败显示 unavailable，不将旧列表重新 flush 回数据库。精确去重与搜索在 Rust 处理，保证 SQL collation 和 MongoDB 下行为一致。

两个记忆工具自动授权、串行执行，沿用预算和 ToolOutput；`GEER_AGENT_TOOLS=off` 或记忆不可用时不声明。界面通过 `interaction::execute` 执行公共记忆命令；TUI 的 F4 面板和共用 React MemoryPanel 保留独立编辑状态与聊天草稿。图形快照增加完整记忆列表、状态和根指令加载标识；前端搜索复用同样的评分、排序与十条上限。列表仅传给 UI，模型按需搜索。

EventHub 的预览现在区分会话删除、记忆单条删除和全局清空，绑定来源端、准确目标与 revision。GUI/Web 的记忆确认均必须匹配预览，任何后续提交或断线使旧预览失效；其他 Web 客户端只同步列表和总数。清空只删除长期记忆记录，现有会话、Trace 和提示文件保持独立生命周期。

## 当前模块职责与协作关系

### 模块职责

| 模块 | 当前职责 | 关键边界 |
| --- | --- | --- |
| [config](/home/lihongyu/projects/geer-agent/src/config/mod.rs) | 配置查找与校验、模型/API/UI 选择、资源额度、跨平台路径辅助 | 不依赖其他业务模块；缺失必要配置会返回错误 |
| [ui](/home/lihongyu/projects/geer-agent/src/ui/mod.rs) | 启动分派、REPL/TUI/GUI/Web 的输入、确认、布局和展示 | 组合点创建 Agent 并注入各自授权回调；错误展示前缀由 UI helper 生成 |
| [ui/repl](/home/lihongyu/projects/geer-agent/src/ui/repl/mod.rs) | 文本提示符、逐行输入、流式输出、颜色、EOF 和终端确认 | 消息与会话操作委托 `interaction::execute`，不引用 `provider`、`agent` 或 `tools` |
| [ui/app](/home/lihongyu/projects/geer-agent/src/ui/app/runtime.rs) | 共用图形工作线程、串行操作、事件缓存、授权与保存判定 | Agent 不跨线程；不依赖 Tauri/HTTP |
| [ui/gui](/home/lihongyu/projects/geer-agent/src/ui/gui/bridge.rs) | 窗口与 IPC 生命周期 | 调用共用图形运行时 |
| [ui/web](/home/lihongyu/projects/geer-agent/src/ui/web/mod.rs) | HTTP、WS、认证、资源与网络额度 | 不执行模型或会话业务 |
| [interaction](/home/lihongyu/projects/geer-agent/src/interaction/mod.rs) | 会话契约、命令解析、共用命令执行、操作结果与错误、状态与用量、诊断缓冲 | 通过泛型 Session 调用业务；不创建 Agent，不含 Ratatui、Tauri 或 ANSI 类型 |
| [agent](/home/lihongyu/projects/geer-agent/src/agent/mod.rs:410) | 实现会话契约，编排模型步骤、工具循环、预算、压缩和检查点 | 持有 Provider、Tools 和 SessionManager，不选择具体界面 |
| [prompt](/home/lihongyu/projects/geer-agent/src/prompt/conversation.rs:24) | 系统提示、协议历史、轮次提交/回滚、压缩边界、快照和原始事件 | 持有模型上下文；目前还包含 GUI 历史展示投影 |
| [provider](/home/lihongyu/projects/geer-agent/src/provider/mod.rs:107) | 单次模型步骤与摘要请求，两种协议转换、流式解析、超时与重试 | 返回 `ModelStep`；不执行工具、不维护整个 Agent 循环 |
| [tools](/home/lihongyu/projects/geer-agent/src/tools/mod.rs:152) | 内置工具注册、参数验证、授权缓存、批次执行、有限输出 | 不依赖模型协议或 UI；未注入确认回调时默认拒绝 |
| [session](/home/lihongyu/projects/geer-agent/src/session/runtime.rs:350) | workspace、活动与停放会话、检查点、存档恢复、删除与保存状态 | 每个会话拥有自己的 Prompt；存储失败时保留内存状态与补写信息 |
| [trace](/home/lihongyu/projects/geer-agent/src/trace/mod.rs) | 模型调用记录契约、流式捕获、状态、用量和脱敏 | 记录单次调用，不代替会话历史 |
| [memory](../../src/memory/mod.rs) | 全局长期记忆的校验、搜索、状态与持久化操作 | 不依赖模型或 UI；数据库成功后更新数量，与会话历史分离 |
| [dao](/home/lihongyu/projects/geer-agent/src/dao/mod.rs) | SQL/MongoDB 适配、会话记录与事件链存取、Trace 读写 | 对上层隐藏后端差异；会话和 Trace 可共享连接 |

### 主要运行时协作

下面表示主要调用与数据流，不是完整的 Rust 静态依赖图：

```mermaid
flowchart TD
    Main["main"] --> UI["ui::run<br/>配置与启动组合"]
    UI --> Repl["ui::repl<br/>文本界面"]
    UI --> Tui["ui::tui<br/>全屏终端"]
    UI --> Gui["ui::gui<br/>Tauri 宿主"]
    UI --> Web["ui::web<br/>HTTP/WS 宿主"]
    Gui --> App["ui::app<br/>共用工作线程与事件缓存"]
    Web --> App
    UI -->|"终端路径创建"| Agent["agent::Agent"]
    App -->|"工作线程创建"| Agent
    Repl --> Execute["interaction::execute<br/>消息、会话与记忆命令"]
    Tui --> Execute
    App --> Execute
    Execute -->|"interaction::Session"| Agent
    Agent --> Provider["provider<br/>单次模型步骤"]
    Agent --> Tools["tools<br/>授权与工具执行"]
    Agent --> Sessions["session::SessionManager<br/>活动与停放会话"]
    Sessions --> Prompt["prompt::Prompt<br/>各会话的模型上下文"]
    Agent -->|"修改当前会话"| Prompt
    Provider --> LLM["配置的模型端点"]
    Tools --> Local["文件与 Bash 子进程"]
    Sessions --> DAO["dao<br/>SQL 与 MongoDB"]
    Agent --> Memory["memory::MemoryService<br/>全局长期记忆"]
    Tools --> Memory
    Memory --> DAO
    Agent --> Trace["trace<br/>调用捕获与脱敏"]
    Provider --> Trace
    Agent -->|"写入 Trace"| DAO
    DAO --> DB["会话、Trace 与独立记忆表"]
```

`interaction::Session` 是接口，`agent::Agent` 是它的实现。`execute` 到 Agent 的箭头表示运行时通过接口调用，不表示 interaction 导入具体 Agent。REPL/TUI 借用 Session；`ui/app` 在工作线程中持有 Agent，GUI/Web bridge 只传输命令和事件，并读取历史与未保存状态。UI 需要的 status/session_entries 等只读数据仍可直接通过 Session 获取，写操作则共用 execute。

### 不能忽略的双向模块引用

当前代码还不是严格单向的层级结构：

- `interaction` 使用 `session` 的列表和删除数据类型，而 `session::runtime` 调用 `interaction::emit_diagnostic`。
- `session::runtime` 持有具体的 `dao::SessionStore`，而 `dao` 使用 `session` 定义的持久化数据类型。

Rust 可以解析这些同 crate 模块引用；它们不是已经发生的编译错误。架构上的影响是契约、运行逻辑与诊断的边界较难读清。后续可将会话数据类型与运行逻辑细分，并把诊断放到可被会话与界面共同引用的低层模块；当前无需为此引入通用 Repository 框架。

## 一条消息如何经过系统

### 正常对话与工具循环

```mermaid
sequenceDiagram
    participant UI as REPL/TUI/GUI
    participant Execute as interaction::execute
    participant Agent as Agent
    participant Prompt as 当前会话 Prompt
    participant Provider as Provider
    participant Tools as Tools
    participant Store as 会话与 Trace 存储
    UI->>Execute: Input::Message，正文/用量回调
    Execute->>Agent: Session::handle_message(input, callbacks)
    Agent->>Prompt: begin_turn(input)
    Agent->>Store: 尝试保存用户事件与检查点
    loop 直到回答完成或预算要求收尾
        Agent->>Prompt: 检查上下文，必要时压缩
        Agent->>Provider: complete_step(messages, tool_specs)
        Provider-->>UI: 经 Agent 转发正文增量
        Provider-->>Agent: ModelStep 与真实用量
        Agent->>Store: 尝试写入本次调用 Trace
        Agent-->>UI: on_usage
        alt 模型请求工具
            Agent->>Store: 工具批次前检查点，标记状态未确认
            Agent-->>UI: 工具进度文本
            Agent->>Tools: execute_batch_in(workspace, calls)
            Tools-->>UI: 经注入回调请求授权
            Tools-->>Agent: 按原顺序返回工具结果
            Agent->>Prompt: apply_tool_results
            Agent->>Store: 工具批次后检查点
        else 模型回答完成
            Agent->>Prompt: finish_turn
        end
    end
    Agent->>Store: 尝试保存最终会话状态
    Agent-->>Execute: Result
    Execute-->>UI: CommandOutcome::Message 或 CommandError
```

模型请求与工具执行之间的循环只有 `agent` 编排。Provider 返回“模型要求做什么”，Tools 返回“执行得到什么”，Prompt 负责把这些内容写成所选 API 的历史；UI 决定用户看到什么。

当前工具批次把连续的只读操作分组并发，Bash、write、edit 等操作作为顺序边界；授权在派发前完成，结果保留调用顺序。Agent 在两种模型协议之间共用这套编排。

Day12 的 `search-fetch` 沿用这条调用链，当前注册表共 11 个工具。Tools 管工具声明、参数校验、确认和调度；query 管 `ls/glob/rg/search`，file 管 `read/write/edit`，bash 管受限子进程及清理，feedback 管错误和输出预算；新增 [web_access](/home/lihongyu/projects/geer-agent/src/tools/web_access.rs) 管独立 HTTP 客户端、Exa JSON-RPC/SSE、有限响应读取和 HTML 转文本。

`search` 加入只读并行段，在调用时规范化 workspace 和目标，按路径组件检查包含关系；rg 在规范化根目录运行，不跟随目录符号链接，限制文件大小并输出 JSON。解析结果始终返回相对 workspace 的路径，保留完整命中行，可接 `read`。候选命中等 end 事件确认非二进制后才返回，捕获和最终输出都有固定上限。这个范围检查只约束 `search`；已有查询和文件工具的路径行为保持。

`web_search` 和 `web_fetch` 串行执行。前者首次会话确认查询发送给 Exa，后者每次确认完整 HTTP(S) URL（含本机和内网），跨来源重定向再次确认，最多 5 次。确认回调继续由界面注入，网页模块不导入 UI；确认等待不消耗抓取的网络总耗时。HTTP 客户端按需创建并复用，保留证书验证和环境代理，不带模型 API key、Cookie 或认证头。网页结果保留完整来源 URL，标记不可信，文本截断使用网页提示。依赖 `reqwest 0.13`、`encoding_rs 0.8` 与 `html2text 0.17`，标准库不提供 HTTPS、字符集解码或 HTML 解析；选型与边界见 [变更设计](/home/lihongyu/projects/geer-agent/openspec/changes/add-search-web-access/design.md)。

Agent 的运行预算检查轮次数、工具调用数、时间与配置的真实用量额度；已有 guard 对重复调用、连续错误和缺少进展给出提示或触发收尾。UI 的临时 token 估算只用于展示，不代替服务端用量判断硬额度。历史压缩以完整轮次和工具调用/结果边界为单位，Provider 的流式超时与 Bash 的执行超时则各自限制单次操作。

### 所有权与线程

- Agent 拥有一个 Provider、一个 Tools，以及一个 SessionManager。活动会话和停放会话分别拥有 Prompt、workspace 与保存状态。
- 文本 REPL 和 TUI 借用 `&mut impl Session`，共用执行函数也只借用同一个 Session；没有增加另一份对话状态。这使界面不必知道模型协议或工具实现，也可使用假 Session 验证界面行为。
- TUI 用 `Rc<RefCell<_>>` 让绘制与同步确认回调共享屏幕状态；回调在同一线程内执行。
- GUI 在工作线程内创建 Agent，避免把包含非 `Send` 回调的对象移到 Tauri 主线程。输入串行处理，桥接发送带 request ID 的增量及快照；授权应答使用独立通道唤醒等待，不排在当前请求之后。
- 当前各界面保持一次处理一条消息的节奏。模型执行期间接受下一条聊天、取消模型请求等能力，需要另行设计。

### 错误与数据语义

模型错误通常通过 `Result` 交给界面。尚未进入工具批次时，Agent 回滚本轮模型上下文；已经进入工具批次时，保留已有的调用/结果内容，避免后续模型丢失可能产生副作用的操作记录。工具失败和授权拒绝作为工具结果回给模型，不等同于整轮网络请求失败。

模型上下文、原始会话事件与 Trace 是三种数据：

| 数据 | 用途 | 生命周期特点 |
| --- | --- | --- |
| Prompt 的模型历史与摘要 | 下一次请求的输入 | 可以压缩，并按轮次提交或回滚 |
| 原始会话事件与检查点 | 恢复会话、展示完整记录、补写失败存档 | 压缩模型上下文时不删除已记录的完整事件 |
| Trace 请求与响应记录 | 分析每次模型调用、重试、错误和用量 | 按调用记录，删除会话时仍保留 |

会话和 Trace 写入失败会报告诊断，并按当前逻辑降级或保留待补写状态。检查点不提供文件或 Bash 副作用的撤销能力。workspace 决定相对路径和执行目录；现有文件工具也接受绝对路径，首次授权的范围是当前会话中的该工具，不是 workspace 文件系统沙箱。

## REPL 的实际作用与颜色代码归属

### REPL 已是文本 UI

REPL 是 Read–Eval–Print Loop，即读取输入、调用处理逻辑、打印结果的交互循环。Eval 可以委托业务对象，并不要求 REPL 自己实现 LLM 协议或会话算法。

当前 `ui::repl::run` 的职责可分为：

| 工作 | 当前执行者 | 合理归属 |
| --- | --- | --- |
| 读取一行输入、识别 EOF 与无效 UTF-8 | REPL | 文本 UI |
| 把命令文本解析为 `Input` | interaction | 界面无关的命令语义 |
| 把 Input 分派到消息、新建、保存、打开等操作 | interaction::execute | 四种 UI 共用的会话执行 |
| 实际执行消息和会话操作 | Agent 实现 Session | 业务编排 |
| 发请求、解析模型流与工具调用 | Provider | 模型协议适配 |
| 提示符、颜色、stdout/stderr、flush | REPL | 文本 UI |
| 删除确认的终端输入与反馈 | REPL | 文本 UI；预览和已确认删除由共用入口执行 |
| 工具授权的终端输入 | ui/repl/authorization | 文本 UI；策略和授权缓存仍由 Tools 执行 |
| 模型上下文、工具循环、预算和压缩 | Prompt 与 Agent | 会话状态与业务逻辑 |

REPL 中的循环是文本 UI 的控制流：读下一行、显示结果、确认输入与处理退出。模型循环位于 Agent；可复用的命令执行已从各 UI 抽到 interaction。文本 UI 仍需决定何时调用执行入口和怎样显示结果，这些属于界面适配。

### Color 本身没有放进业务层

[ui::repl::Color](/home/lihongyu/projects/geer-agent/src/ui/repl/color.rs) 根据 stdout 是否为终端、`NO_COLOR` 和 `TERM=dumb` 判断是否启用 ANSI，并负责用户提示符、助手提示符和输出颜色。这些都是文本界面职责。

本次把文本循环、颜色与终端确认一起归入 `ui/repl`，移除了顶层模块。只移动颜色会留下标准输入输出、提示符和确认等界面代码，不能完成 UI 目录边界的整理。

`Color` 适合作为文本界面的私有实现，不必成为全项目的主题对象。ANSI、Ratatui Style 和 GUI CSS 的表示不同；可以共享事件的语义，具体颜色由各界面选择。

### 更需要优化的是字符串事件协议

当前 `Session::handle_message` 的 `on_delta` 参数只有 `&str`。它接收的不仅是模型正文，还包括核心产生的通知：

| 内容 | 产生位置 | 界面如何识别 |
| --- | --- | --- |
| 模型正文增量 | Provider | 通常按助手正文处理 |
| 工具开始通知 | Agent | 文本 REPL/TUI 判断前缀，GUI 解析完整工具标记 |
| 自动压缩成功、失败和溢出后压缩通知 | Agent | TUI 识别更多前缀，文本 REPL 和 GUI 没有相同的完整分类 |
| 预算收尾失败提示 | Agent | TUI 有专门前缀识别 |
| `retry 1/5...` 等重试提示 | Responses Provider | 与正文共用回调，界面没有独立的重试类型 |

对应实现见 [Agent 工具循环](/home/lihongyu/projects/geer-agent/src/agent/mod.rs:873)、[TUI 进度识别](/home/lihongyu/projects/geer-agent/src/ui/tui/mod.rs:560)、[GUI 工具进度识别](/home/lihongyu/projects/geer-agent/src/ui/app/commands.rs:16) 和 [Responses 重试](/home/lihongyu/projects/geer-agent/src/provider/openai/responses.rs:130)。

这是已经存在的展示耦合：改一句核心文案就可能改变 UI 分类；三个界面的分类范围也不一致。如果模型恰好在一次回调中输出同样的前缀或完整标记，也有被误判为进度的可能。GUI 对完整标记的校验更严格，但仍然需要从字符串猜测来源。

重试文案走正文回调，不代表它已写入模型历史；模型历史来自最终 ModelStep。问题在于临时展示通道混合了状态和正文。单独移动颜色文件不能解决这一点。

## 优化目标与设计选择

### 优先级

以下优先级用于安排架构工作，不代表已验证的故障等级：

| 优先级 | 问题 | 状态与处理 |
| --- | --- | --- |
| 已实施 | 文本界面在顶层，UI 目录边界不完整 | 整体迁入 `ui/repl` |
| 已实施 | 三种 UI 重复调用同一组会话操作 | 共用 `interaction::execute`，返回操作结果与错误类别 |
| 已实施 | Tools 默认直接读取终端授权 | 终端输入迁入 UI；三条启动路径注入确认，Tools 默认拒绝 |
| 高 | 核心通知与模型正文共用字符串回调 | 后续引入带语义的模型事件，UI 不再通过中文前缀分类 |
| 中 | 会话接口仍返回已格式化字符串 | 已增加操作结果外壳；后续逐项结构化保存、压缩等报告 |
| 中 | Prompt 包含 GUI 展示投影与系统展示文案 | 后续将展示投影交给会话读取逻辑 |
| 后续 | Agent 文件汇集循环、预算、压缩和 Trace 等职责 | 按现有能力拆成普通 Rust 子模块，保留算法和入口 |
| 后续 | 契约与运行模块存在双向引用 | 拆清数据、运行与诊断依赖；治理与导读本次已同步 |

### 目标职责边界

```mermaid
flowchart TD
    Entry["main / ui::run<br/>启动组合"] --> Views["ui::repl / ui::tui / ui::gui<br/>输入、确认、渲染"]
    Views --> Contract["interaction<br/>命令、会话操作、语义事件"]
    Agent["agent<br/>会话与工具编排"] -->|"实现"| Contract
    Entry -->|"构建并连接"| Agent
    Agent --> Provider["provider<br/>协议请求与协议事件"]
    Agent --> Tools["tools<br/>执行与授权策略"]
    Agent --> Session["session / prompt<br/>会话状态、模型上下文与原始记录"]
    Session --> Storage["dao / trace<br/>存取、记录与诊断"]
    Agent --> Storage
```

这个图描述目标职责；仍需在文件级区分 `session` 的数据契约与运行逻辑，不能据此宣称所有顶层模块引用已经单向化。具体 UI 可在启动与桥接处组合 Agent，但模型和工具核心不导入具体 UI。

本次模块布局：

```text
src/
  main.rs
  interaction/
    mod.rs                 # Session、命令解析、状态与用量
    command.rs             # execute/save、CommandOutcome、CommandError
    command_tests.rs       # 共用执行语义回归
    tests.rs               # 输入解析回归
  ui/
    mod.rs                 # 保留启动组合
    repl/
      mod.rs               # 输入循环与执行结果渲染
      color.rs             # 文本界面私有颜色
      authorization.rs     # 从 Tools 移入的终端授权输入
    tui/                   # 保留现有实现
    app/                   # 共用图形会话运行时
    gui/                   # Tauri 宿主
    web/                   # HTTP/WS 宿主
    frontend/              # 唯一 Bun + React TS 包
  agent/                   # 保留业务入口与已有 guard
  provider/                # 保留两种 API
  tools/                   # 保留工具注册、验证、执行和授权缓存
  prompt/
  session/
  trace/
  dao/
```

这是本次实施后的主要结构；省略了现有子模块与测试夹具。没有增加 crate 或依赖，目标图里的结构化模型事件尚未实施。

### 已实施的共用执行接口

[interaction::execute](/home/lihongyu/projects/geer-agent/src/interaction/command.rs) 接收 `&mut impl Session`、Input 和正文/用量回调，统一调用消息、新建、保存、压缩、列表、打开、workspace 与删除能力。它不持有 Provider、Tools 或另一份 Prompt。

| 共用结果 | UI 如何使用 |
| --- | --- |
| `NewSession { session_id, reset }` | 文本界面保留 /reset 文案；TUI/GUI 使用自己的换行与提示 |
| `Opened { message, session_id }`、`WorkspaceChanged` | 根据操作结果刷新会话身份、清理对应 UI 草稿 |
| `Saved`、`Compacted`、`Sessions`、`Workspace` | 渲染已有报告或列表；本次保留其内部文本格式 |
| `DeletePreview` | 仅展示预览，等待用户确认；没有发生删除 |
| `Deleted` | 展示报告并刷新面板；直接构造的请求也校验、去重完整 UUID |
| `Message`、`Exit`、`Help` 等 | 标识操作完成或 UI 请求；Exit 本身不决定窗口关闭或保存策略 |

失败返回 `CommandError { operation, message }`，业务入口只标记操作类别与原因；“模型请求失败”“会话恢复失败”等展示前缀由 [ui/commands.rs](/home/lihongyu/projects/geer-agent/src/ui/commands.rs) 生成。正文/用量回调仍同步、按原顺序执行，回调错误继续返回调用方。

EOF、终端退出和 GUI 关闭复用 `interaction::save`。具体退出策略仍由 UI 控制：文本/TUI 在退出循环后保存，GUI 沿用现有关闭保存和失败确认机制。

TUI 的面板读取 `Session::session_entries`，绘图读取 status；这些是 UI 查询。面板的打开和删除操作转换为 Input 后使用 execute。GUI commands 只把共用结果适配成现有窗口通知、错误与删除弹窗数据，bridge 的线程、request ID、快照与授权应答通道沿用现有实现。

### 后续：结构化模型事件

建议在 `interaction` 定义界面无关事件。下面是接口草案，字段在对应 OpenSpec design 中确定；现有持久化类型 `session::SessionEvent` 继续表示数据库事件，二者用途不同。

```rust
pub(crate) enum InteractionEvent<'a> {
    AssistantDelta(&'a str),
    ToolStarted {
        name: &'a str,
    },
    Retry {
        attempt: u32,
        max_attempts: u32,
    },
    CompactionCompleted {
        old_items: usize,
        before_tokens: u64,
        after_tokens: u64,
        after_overflow: bool,
    },
    CompactionFailed {
        message: &'a str,
    },
    FinalizationFailed {
        reason: &'a str,
    },
}
```

保留 `Session` 的泛型异步调用方式，将文本回调逐步改成事件回调；现有用量回调可先保留，减少一次接口迁移的范围。类型里不放 ANSI、组件、窗口或布局信息。

这里的 ToolStarted 表示开始处理工具请求，不表示已经授权或执行成功；授权决定与执行结果仍走现有业务路径。

Provider 的协议事件另定义在 `provider`，例如正文增量和重试次数，由 Agent 映射为 InteractionEvent。Provider 不应为了上报重试去引用上层 `interaction` 或 UI。重试策略与次数仍由 Provider 控制，展示文案由界面生成。

文本 REPL 将事件格式化为现有文本；TUI 根据事件类型选择消息角色；GUI bridge 转为可序列化的 GuiEvent，并附上 request ID。借用的 `&str` 只在同步回调期间使用，GUI 在跨线程/IPC 边界转换为拥有所有权的 String，不能缓存借用。

回调继续返回 `io::Result`，错误交给 Agent 现有的错误路径处理。接口迁移必须检查断管、绘制失败和工具批次后的历史提交，避免在重构中吞掉错误或遗漏已有副作用记录。

### 已实施的授权策略与授权界面

[Tools 的授权逻辑](/home/lihongyu/projects/geer-agent/src/tools/mod.rs) 决定哪些工具需要确认、授权缓存何时生效；[文本授权输入](/home/lihongyu/projects/geer-agent/src/ui/repl/authorization.rs) 使用 stdin/stdout。二者已分开：

- Tools 保留工具开关、授权缓存、执行前检查和拒绝结果。
- `Tools::new` 的默认确认回调返回拒绝。
- 文本 REPL、TUI、GUI 在各自组合入口注入确认回调。
- 终端授权函数位于文本 UI，保留明确同意与非交互默认拒绝的行为。

文本路径由 `ui::run` 在调用 REPL 前通过 `Agent::set_confirm` 安装文本界面的确认回调。REPL 循环继续只借用 Session，不因迁移授权而导入 Agent 或 Tools。

现有 `FnMut(&str) -> io::Result<bool>` 同时支持会话授权和逐次网页确认。Tools 的提示说明实际授权范围，GUI 的通用文案不承诺授权一定会缓存。若后续需要更丰富的 GUI 展示，再由 Tools 定义包含工具名、操作详情和授权范围的请求类型，UI 生成 `[y/N]` 等文案。授权决定不能由“事件已显示”或模型文字代替。

### 会话结果与展示投影

`Session` 已有 `session_entries`、DeletePreview 和 DeleteReport 等结构化返回，但 `flush`、`compact`、`open`、`set_workspace` 和文本列表仍返回展示字符串。会话管理也负责时间、状态及列表行的格式化。

边界不以“是否返回 String”为唯一标准：模型正文、工具反馈和错误原因本来就是文本；跨界面共用的会话标题也可以留在中立的读取模型。需要迁出的主要是颜色、布局，以及必须解析固定文案才能判断业务状态的逻辑。

先复用现有结构化数据，再按操作增加小型结果类型，例如保存报告中的 UUID 与 SaveStatus、压缩结果中的前后 token 数、打开结果中的 workspace 与工具状态。UI 决定换行、标签和列表布局；业务返回能供判断的数据。不要一次性设计通用命令执行框架；三个界面的输入、确认与退出行为仍可分别实现。

`Prompt::transcript` 目前根据原始事件生成 GUI 历史，并包含回滚、压缩等中文展示文案。后续可把纯投影移到 `session::projection`：Prompt 提供原始事件，SessionManager 调用投影，Agent 再提供读取接口；Prompt 不反向引用 SessionManager 或 UI。

先保留事件来源、快照格式和现有按 feature 保存展示记录的策略，避免仅为搬代码而扩大默认内存占用。GUI 的完整历史与模型压缩上下文继续分离。让 TUI/文本界面也展示完整旧历史属于额外的可见能力，需单独规划。

### 按能力拆分大型文件

等事件和授权边界稳定后，可从 Agent 的现有实现中依次提取 `runtime`（预算与指标）、`turn`（工具循环与收尾）、`compaction`、`tracing` 子模块；`mod.rs` 保留构建与 Session 实现，已有 `guard` 继续使用。

Session 可先把数据类型与运行管理分开，DAO 只引用数据契约；诊断暂存与输出也可独立于会话操作契约。只有需要并发请求或更换执行线程时，再将现有线程局部诊断缓冲改为显式注入，避免提前增加管理器或全局事件总线。

这些拆分的目的在于缩小阅读和修改范围。它们不改变协议历史、压缩算法、数据库格式或工具调度规则。

## 分阶段实施与验收

本次共用命令执行、文本 UI 迁移与授权 I/O 分离通过 OpenSpec 规划后实施，使用 `skip_specs` 保持现有行为。后续正文分类、通知展示或授权范围发生可见变化时，应更新对应行为契约。

当前主 spec 包含 `tool-loop`、`session-persistence`、`llm-trace`、`tui`、`gui`；`repl-chat` 等能力还存在于已完成但未归档的 change 中。建变更前核对 CLI 状态，复用能力名称，不重复创建近义 spec。

| 阶段 | 一个可运行的切片 | 验收重点 |
| --- | --- | --- |
| 已完成 | 共用 execute 与操作结果，整体迁入 `ui/repl`，CLI 授权移入 UI | 三种界面复用同一 Session；命令、流式、EOF、删除与授权保持原样 |
| 后续 1 | 替换混合字符串回调，贯通 Provider → Agent → 四种 UI 的语义事件 | 模型正文不被当作工具标记；工具、重试、压缩和收尾通知各自分类 |
| 后续 2 | 选一个会话操作返回结构化报告，再整理历史投影 | 四种界面消费同一业务数据；旧存档、压缩、待补写、删除与关闭语义不回归 |
| 后续 3 | 按已明确的职责提取 Agent/Session 子模块 | 普通启动和两种 API 路径仍可运行；缩小阅读与修改范围 |

后续每个切片独立规划、实现与验证；各会话报告也应分次迁移。

### 有意义的验证场景

| 范围 | 应验证的行为 |
| --- | --- |
| 文本界面 | `NO_COLOR`、`TERM=dumb` 或输出接管道时无 ANSI；空行不发请求；EOF 保存并退出；无效 UTF-8 后仍能继续 |
| 事件语义 | 模型正文恰好包含完整工具标记仍是正文；核心产生的相同工具通知是工具事件；两者展示文案相同也可区分 |
| 重试 | 无正文时可按原策略重试；已有部分正文时不重试；重试提示不进入下一轮模型历史 |
| 工具与错误 | 调用与结果配对、只读批次结果顺序、拒绝不执行、预算收尾；工具批次前后请求失败保留原有提交/回滚语义 |
| 会话 | 不同 workspace/history 隔离；压缩后能恢复完整记录；保存冲突与失败可补写；删除保留 Trace |
| GUI/TUI | 正确消费事件与用量；迟到 GUI 增量不覆盖新请求；授权可以应答；退出恢复终端或报告保存失败 |

已有相关回归包括 [tool_loop](/home/lihongyu/projects/geer-agent/tests/tool_loop.rs)、[responses_retry](/home/lihongyu/projects/geer-agent/tests/responses_retry.rs)、[session_compaction](/home/lihongyu/projects/geer-agent/tests/session_compaction.rs)、[gui_selection](/home/lihongyu/projects/geer-agent/tests/gui_selection.rs) 和 [config_paths](/home/lihongyu/projects/geer-agent/tests/config_paths.rs)。`gui_selection` 只在未启用 `gui` feature 时运行，不能把它当作原生窗口验收。

### 检查入口

先 fmt，再按改动选择受限测试，最后 clippy；收工执行 `make check`。所有测试通过 [Makefile](/home/lihongyu/projects/geer-agent/Makefile) 和受限入口运行，不裸跑测试二进制、Cargo 测试或 Bun 前端测试：

```sh
make fmt
make test TEST_ARGS='--test tool_loop'
make test TEST_ARGS='--test responses_retry'
make test TEST_ARGS='--test session_compaction'
make clippy
make check
```

按改动选择相关项并顺序执行。若修改 GUI 事件或前端，补充 `make frontend-check`、`make frontend-test`，构建前端后再进行带 `gui` feature 的 Rust 检查和原生交互验收。

### 本次验收记录

- `make test-safety` 已通过，实际验证资源限制、独立 tmpfs 与正常/失败/超时/SIGKILL/OOM 等回收路径。
- 共用 interaction 的 10 项测试、GUI 命令的 4 项测试、TUI 的 14 项测试通过。
- `tool_loop` 的 28 项和 `session_compaction` 的 8 项集成回归通过，覆盖两种 API、命令不混入对话、workspace 隔离、EOF 保存、持久化与管道删除。
- 授权相关的 4 项单元回归与非交互 Bash 拒绝单项通过，覆盖独立工具授权、撤销、恢复会话后清空授权；三条 UI 启动路径均显式注入确认回调。
- `make check` 通过：217 项测试通过，隔离检查通过，默认构建 clippy 无警告。
- `make clippy-all` 通过：受限服务中构建 GUI 前端，并检查 `--all-targets --features gui`，实际桥接代码编译通过且 clippy 无警告。
- `openspec validate centralize-ui-session-commands --strict` 通过；本地文档链接与变更空白检查通过。
- 原生 GUI 窗口与真实 TUI 终端的键盘/鼠标手工验收尚未执行；无 GUI 构建下的 `gui_selection` 回归不等同于原生窗口验收。

涉及 Shell、PATH、超时或清理代码时，按项目约束先 `make test-safety`，再受限运行缺失命令单项与 `process_safety` 回归，最后完整检查。必须确认实际资源和临时目录隔离有效；安全入口失败时先解决隔离问题。

Day12 `search-fetch` 增量通过最终 `make check`（231 项 Rust 测试，1 项真实联网测试默认忽略）、23 项 GUI 前端测试、类型检查和带 GUI feature 的 clippy。真实 Rust 客户端访问 Exa 与 Rust 官方文档成功；REPL/TUI 通过本机 mock 和伪终端验证逐次与重定向授权。原生 GUI 人工检查步骤及详细结果见 [search-fetch 验收记录](/home/lihongyu/projects/geer-agent/openspec/changes/add-search-web-access/verification.md)。

## 保留的设计与后续文档维护

继续保留单一业务实现、两种模型协议、按会话拥有 Prompt、共用图形工作线程和现有工具注册表。REPL 可以作为文本 UI 名称继续使用，无需因名称歧义删除这个运行模式。泛型 Session 与回调已适合当前单线程节奏，不必改成对象安全的 `dyn Session`。

本次边界调整不需要新的业务分层目录、依赖注入容器、全局主题框架或事件总线。数据库依赖的编译成本可以在有测量数据和明确需求时单独评估，不在 UI 重构中移除现有后端。

相关历史设计可结合阅读：[REPL 与 Agent 分离](/home/lihongyu/projects/geer-agent/openspec/changes/separate-repl-agent/design.md)、[TUI 与用量面板](/home/lihongyu/projects/geer-agent/openspec/changes/add-tui-usage-panel/design.md)、[桌面 GUI](/home/lihongyu/projects/geer-agent/openspec/changes/add-desktop-gui/design.md)。早期分离方案曾明确保留字符串前缀着色；现在已有三个界面消费者，事件语义的维护成本已更明显。

本次已同步 [AGENTS.md](/home/lihongyu/projects/geer-agent/AGENTS.md) 中的项目状态和入口约定、[OpenSpec 配置](/home/lihongyu/projects/geer-agent/openspec/config.yaml) 中的现状及受限检查入口，以及 [源码导读](/home/lihongyu/projects/geer-agent/doc/learn-rust/04-project/01-code-map.md) 的调用图。本文作为设计入口，行为验收继续由主 spec、对应 change 和测试共同约束。
