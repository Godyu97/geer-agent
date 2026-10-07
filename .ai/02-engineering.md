# 工程边界

模块当前事实在 [docs/mod](../docs/mod/00-index.md)。教学阅读可查 [源码地图](../doc/learn-rust/04-project/01-code-map.md)，架构脉络可查 [架构文档](../docs/design/architecture.md)；其中「目标」「后续」与历史验收不能直接当作现状。

## 架构与依赖方向

单一 Rust 二进制 crate。[main.rs](../src/main.rs) 只进入 `ui::run`；[ui/mod.rs](../src/ui/mod.rs) 加载环境、选择 UI 并组合 Agent。普通 `auto` 按 stdin/stdout 是否为终端选择 TUI/REPL；`desktop-gui` 把 auto/gui 解析为 GUI，并拒绝终端/Web 模式。

`config` 不依赖业务模块；`provider` 不依赖 tools/agent/ui，`tools` 不依赖 provider/agent/ui。Agent 组合 prompt、provider、tools、session、memory、DAO 和 trace，实现 `interaction::Session`。`interaction` 可用会话/记忆数据类型，不导入具体 Agent/UI；各界面的写操作经 `interaction::execute`，绘图查询可读 Session。

终端在 current-thread Tokio runtime 中使用 Agent；[ui/app](../src/ui/app) 在专用工作线程创建 Agent，串行处理图形命令，共用事件缓存、授权 Gate 与关闭保存。GUI/Web 宿主只实现窗口/IPC 或认证/HTTP/WS，不能复制 Agent 编排。工具内部的有界并行由 tools 调度，不能据此并行修改同一会话。

## 契约与事实来源

已同步契约以 [主 spec](../openspec/specs) 为准；进行中 change 的 proposal/specs/design/tasks 表示该变更的目标、决定和实施记录，是否落地仍须对照代码/测试。主 spec 尚未覆盖全部已实施能力，定位方法见 [工作流](03-workflow.md#openspec-路由)，不能把没有归档误判为没有实现。

Rust 图形事件/快照由 [ui/app/events.rs](../src/ui/app/events.rs) 及其引用数据类型序列化；[commands.rs](../src/ui/app/commands.rs) 组织共享结果。[前端 protocol.ts](../src/ui/frontend/src/protocol.ts) 是人工维护的 TS 消费类型，不是从 Rust 自动生成的 schema。修改字段、默认值、revision 或确认状态时，一起核对 Rust 生产端、TS reducer 和两个 HostAdapter，不另抄完整字段表。

[SessionRuntime](../src/session/runtime.rs) 与 [Prompt 快照](../src/prompt/conversation.rs) 定义恢复数据；DAO 屏蔽 SQL/MongoDB 差异，SQL 迁移在 [dao/sql.rs](../src/dao/sql.rs)。会话 revision、事件链、workspace 和模型/API/端点身份均参与兼容性判断；保存冲突或失败必须保留真实状态。Trace/会话存储前的脱敏依赖已配置秘密串，不代表所有用户内容自动去敏。

模型协议通过 provider 支持 Responses/Chat Completions，不让 UI 或 tools 依赖 API 请求类型。跨模块公开契约也包括 CLI、配置、持久格式、事件和失败语义；契约内修复保留行为，有意改变契约按现有 OpenSpec 流程。

## 指令与工具授权

[ProjectInstructions::load](../src/prompt/mod.rs) 只读取当前 workspace 根 `AGENTS.md`，缺失/空内容表示无指令，其他读错明确失败；SessionManager 在启动、切换/恢复时加载，Agent 每条消息前再刷新。不会自动合并祖先/子目录或加载 Markdown 链接，`.ai` 与模块正文仍须按任务读取；这是 geer-agent 的静态实现事实，不是其他 Agent 客户端的加载保证。

[tools](../src/tools/mod.rs) 先校验参数再授权，可整体关闭。Bash、正文搜索、网页搜索和文件操作首次按工具授权；网页抓取每次确认 URL，跨来源重定向再确认；可用的记忆工具自动执行。文件工具接受 workspace 相对路径、绝对路径和 `~/`，已有授权按工具复用；只有正文 `search` 强制规范化后位于 workspace 内。它们不是统一的路径沙箱，当前语义和对应 change 见 [工具模块](../docs/mod/tools/01-implementation.md)。这些运行时权限不扩大编码任务本身的授权。

## 前端共用边界

唯一包 [src/ui/frontend](../src/ui/frontend) 使用 Bun 1.4.2，只维护 `bun.lock`。App/reducer/protocol/CSS 共用；`HostAdapter` 注入 desktop/web 能力，Tauri IPC、目录选择与系统剪贴板留在 desktop，HTTP/WS、重连与浏览器复制留在 web。安装冻结锁文件且安装脚本并行 1，不强制改变 Vite/tsc/Vitest 的 Node 兼容运行方式。变更前读 [README 前端约定](../README.md#共用-react--typescript-的管理方式)。

## 生成与维护边界

| 产物/路径 | 维护来源与方式 |
| --- | --- |
| `src/ui/frontend/dist/gui`、`dist/web` | [Vite 配置](../src/ui/frontend/vite.config.ts) 按模式选择宿主；`make frontend-build` 从共享源码生成，不直接编辑产物。 |
| Rust 构建/Tauri 产物 | [build.rs](../build.rs) 追踪 UI 资源，web 要求 index.html；Tauri 消费 [配置](../src/ui/gui/tauri.conf.json) 与 capabilities。默认 Cargo 不依赖前端；平台要求见 README。 |
| `Cargo.lock`、`bun.lock` | 各由 Cargo/Bun 随授权的依赖变更维护；不手改解析结果，不增加 npm 等第二套锁文件。 |
| `.agents/skills/openspec-*` 与 `.openspec-target` | `openspec update` 刷新，不手改生成 skill；项目规划规则留在 [config.yaml](../openspec/config.yaml)，不建立 openspec/AGENTS.md 或 project.md，不将 context/rules/skill 正文复制进产物。 |
| `.ai/catalog.json` 与模块索引 | 接入后是人工维护事实，按 [维护手册](04-documentation.md) 同步，不反复运行 bootstrap 覆盖手册。 |

`embed-env` 会在编译时读取 `.env` 并嵌入程序，只能显式选择；Make 全界面入口指定 `gui,web` 而非 `--all-features`。仓库中的图标、capabilities、HTML、CSS、构建配置和测试夹具都是实际维护范围，不能仅按源码后缀忽略。
