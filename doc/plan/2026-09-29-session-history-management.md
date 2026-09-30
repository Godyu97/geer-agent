# Sessions 历史管理与展示标题

## 目标与边界

沿用现有 Prompt、SessionManager、SessionRuntime 和 SQL/MongoDB DAO，提供单条/批量删除、GUI/TUI 勾选管理，以及首条用户消息生成的展示标题。UUID 仍是唯一身份；标题不保存到数据库、事件或 PromptSnapshot。删除快照与消息事件，保留 Trace。删除当前会话成功后，在原 workspace 创建新 UUID 空会话并清空授权。

本次不增加分页、回收站、手工重命名、搜索或数据库迁移。近期存档继续最多 20 条；全选只覆盖当前列出的条目。

## 展示标题

首条输入是原始事件中的第一条 user 消息，不包括命令、摘要或后续消息。移除控制字符，合并空白，取前 20 个 Unicode 字符，超长追加省略号；空会话显示“新会话”。持久化会话从已发布检查点的完整事件链读取，保持压缩与重启前后一致；读取失败回退短 UUID 并发出诊断。

Prompt 在内存中保留首条输入以应对 pending events 清理和上下文压缩，恢复时从已验证事件重建。列表投影使用进程缓存，缓存以 UUID 和事件头标识关联，避免 GUI 的当前/全部列表反复读取同一事件链。SessionEntry 和 SessionStatus 增加展示标题，列表和状态区域使用同一生成规则；完整 UUID 仍可查看。标题按普通文本呈现。

## 删除与错误语义

Session 公开结构化列表与 delete_sessions(ids)，返回每个唯一 ID 的结果：deleted、absent、failed、cleanup_pending，以及当前会话是否被替换。只接受完整 UUID，不接受标题、前缀或通配符。空参数和非法命令给出用法。

批量逐项执行；SQL 每个会话在事务中删除快照和所有关联事件，单项失败不影响其他项。MongoDB 先删除快照，再删除事件，不依赖副本集事务；确认快照已不存在但事件清理失败时返回 cleanup_pending。重复删除同一 ID 仍会清理残留事件。没有持久化时只删除进程内会话。

确认删除或已不存在后，移除对应内存状态、待保存队列和标题缓存。当前会话最后处理，成功后直接替换成原 workspace 的新状态，不能调用会保存旧状态的 new_session。失败时保留原状态。已删除会话不得因 flush、退出或旧 revision 保存复活。删除不要求 workspace 仍存在，也不校验模型兼容性。

## 命令与界面

- `/delete <id> [id...]` 请求一次确认；`/delete --yes <id> [id...]` 直接执行。交互 REPL 默认为取消，管道无 --yes 时只提示，不把后续输入读作确认。
- GUI 侧栏增加管理模式、复选框、全选/取消全选、计数和删除所选。管理状态点击条目只改变选择；正常状态点击仍打开。复用 gui_submit、工作队列和忙碌锁，返回结构化确认与删除报告，确认后重新提交冻结的 UUID 集合。
- TUI 的 F3、/sessions 和 /sessions --all 打开管理面板；上下/PageUp/PageDown 移动，Space 勾选，a 全选/取消全选，Delete 删除选中项，Enter 打开高亮项，Tab 切换当前/全部，Esc 关闭。删除确认默认取消；确认结束仍显示管理面板和逐项结果。
- 选择状态按 UUID 保存。切换范围、workspace、打开会话或退出管理时清空；刷新只保留仍存在的选项。全选不包含隐藏范围或未加载存档。
- 确认显示数量、标题、完整 UUID、Trace 保留说明，以及当前会话替换提示。删除结果支持重试失败项（包含清理告警）。模型/工具运行和关闭时禁止删除。
- 删除其他会话或取消操作保留聊天草稿；当前会话被替换时清空消息区和草稿、本轮用量与授权，进程累计用量保持。

## 实施与验证

通过 OpenSpec CLI 建立 improve-session-history-management 并先完善中文产物。先实现展示投影与 DAO，再完成共享命令、REPL、GUI、TUI；不增加依赖，优先使用现有 serde、uuid、SeaORM/MongoDB、Ratatui 与 React。

测试覆盖中英文/emoji/空白/控制字符/同名标题、双协议压缩与恢复、内存和持久化删除、当前与跨 workspace 删除、重复删除、Trace 不变、存储故障和重试、旧 revision 不复活，以及选择/确认/取消/范围切换/忙碌/窄屏。使用隔离临时数据库和模拟模型运行 REPL、PTY/TUI 与本机 GUI 验收。外部数据库仅在显式测试配置可用时运行，未实测项目单列。

质量门依次为 cargo fmt、Rust 默认及 GUI 测试、前端 check/test/build、默认及 GUI Clippy、OpenSpec strict 和 git diff --check。默认不打开 embed-env。

## 验收记录（2026-09-30）

功能实现已完成，README 已补充入口、快捷键、标题规则、删除与重试语义。

| 验证 | 结果与覆盖范围 |
| --- | --- |
| `cargo fmt --all`、`cargo fmt --all -- --check` | 通过 |
| `cargo test` | 默认构建通过；148 个单元测试及 42 个集成测试 |
| `cargo test --features gui` | GUI 构建通过；150 个单元测试及 42 个集成测试 |
| `cargo clippy --all-targets`、`cargo clippy --all-targets --features gui` | 通过，无警告 |
| 前端 `npm run check`、`npm test`、`npm run build` | 通过，3 个测试文件、19 项测试 |
| `cargo build --features gui` | 最新前端产物嵌入并构建通过 |
| `openspec validate improve-session-history-management --strict`、`git diff --check` | 通过 |

默认/GUI 测试中的外部数据库契约入口在没有对应测试 URL 时直接返回；以上通过数量不代表外部数据库已实测。

新增测试验证了 Unicode 标题、首条输入在压缩/恢复后的稳定性、同名会话按 UUID 区分、快照未增加标题字段、SQL 事务回滚、当前与停放会话的待补写数据移除、旧 revision 无法重建检查点、跨 workspace/不可恢复存档的删除，以及当前删除后的授权与用量重置。`tests/session_compaction.rs` 使用模拟 Chat Completions 和 Responses 服务，在多个真实进程间验证标题恢复、管道未确认不吞下一命令、去重批量删除、退出补写不复活和 Trace 保留。

真实 PTY 使用临时 SQLite 和模拟模型执行 REPL、TUI。REPL 验证了空确认取消、一次确认多个 UUID、删除当前会话后退出保存新会话。TUI 验证了 F3 打开、Tab 切换范围、a 全选、Delete 预览、Enter 取消后草稿仍在、y 确认和返回聊天；退出后直接查询数据库，旧快照与消息均不存在，新空会话存在，原 Trace 数量保持。

GUI 在 Chrome 中加载实际生产构建和隔离的模拟 IPC：验证同名会话单独勾选、全选、范围切换清空选择、取消保留草稿、确认显示完整 UUID、包含当前会话的批量删除后清空草稿和消息。自动化前端测试还覆盖失败项保持选择、残留清理重试、忙碌禁用以及打开当前会话时清空选择。浏览器模拟 IPC 不能单独当作 Tauri 原生窗口已验证。

本机 Wayland 原生窗口用独立临时数据库、`GDK_BACKEND=wayland` 和 AT-SPI 点击完成操作验收（工作目录 `/tmp/geer-gui-native-vxoebfvb`）。两个同名会话 `677dbc0d-1aab-4c35-86bc-2d793b192ccf`、`10843750-d79e-43e9-ae66-44e986f10866` 经管理模式勾选、确认框展示完整 UUID、取消后当前会话仍打开、切换「全部」后复选框全部未勾且「删除所选」禁用；再勾选含当前会话的两条并确认删除。删除后窗口列出替换「新会话」`e77b69cb-e1b0-4fc7-a1be-f265121a5de6`，主区回到「从这里开始」，composer 为空。SQLite 中两条测试会话快照与消息均不存在，对应 `llm_traces` 仍各 1 条；未保存的空替换会话不落库。WebView 合成画面无法用截图工具取证，操作证据为 AT-SPI 树；键盘合成无法写入输入框，草稿保留仍以浏览器模拟 IPC 为准。真实模型、Windows、外部 PostgreSQL/MySQL/MongoDB（包括真实 MongoDB 分阶段清理故障）未实测。
