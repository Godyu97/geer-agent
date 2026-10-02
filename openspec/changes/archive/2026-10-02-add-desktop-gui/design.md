# Design

## Context

见 proposal.md。`main.rs` 启动 `ui::run`，后者先构建 Agent，再选 TUI 或 REPL；`Config::load` 在 Agent 构建时读取 `.env`。`interaction::Session` 已承载命令、流式文本和用量回调；工具授权是同步 `FnMut`；`Prompt` 拥有模型历史和待保存事件。会话恢复已读取并验证原始事件链，但目前不将事件转为界面历史。Tauri 窗口要求主线程事件循环，现有 Agent 含非 `Send` 回调。

## Goals / Non-Goals

**Goals:** 保留单一二进制和已有 Agent 所有权；可选 GUI 编译、运行时界面选择；复用已保存事件提供完整历史。

**Non-Goals:** 不重写模型或工具循环，不增加另一套会话数据库，不提供安装器和图形化配置编辑。

## Decisions

1. **编译与配置**：`gui` Cargo feature 只开启 Tauri、构建辅助及 GUI 模块；`GEER_AGENT_UI=auto|gui|tui|repl` 在启动时决定入口。将现有 dotenv/内嵌环境加载提取为可重入步骤，在界面选择前执行，`Config::load` 在构建 Agent 时再次调用并解析模型配置。外部 `.env` 沿用可执行文件同级、本地 Cargo 项目、用户目录的既有优先顺序；进程环境变量优先。默认 `auto` 保留 TTY/管道选择。标准库没有跨平台 WebView 和窗口事件循环，选 Tauri 2；React、Vite 与 `react-markdown` 分别承担复杂界面状态、资源构建和安全 Markdown 呈现。Iced/egui/Slint 要切回原生绘制，Dioxus 要改为 RSX，均与用户确定的 React/TypeScript 路线不符。前端锁文件固定版本。
2. **位置与构建**：GUI Rust 桥接、配置放 `src/ui/gui/`，前端放其 `frontend/`。使用单个 `geer-agent` 二进制与可选 `gui` feature；`main` 的同步部分选择界面，TUI/REPL 路径进入现有单线程 Tokio runtime。GUI feature 的构建辅助只在启用时运行，包含编译完成的本地前端静态资源。默认 `cargo build` 不要求 GTK、WebKitGTK 或 Node。Windows 保留控制台子系统，因为同一可执行文件也运行 TUI。
3. **异步与所有权**：Tauri 事件循环在主线程；工作线程内部新建 Tokio current-thread runtime、Agent 和授权回调，避免移动非 `Send` Agent。普通输入与会话操作串行送入工作线程；Tauri commands 返回已排队结果，有序 Channel 向单个窗口传递增量、用量、授权与快照。命令和回调结果转成界面可展示的错误文字，不将非 `Send` 错误跨线程传输。每条事件带操作/会话标识；断线丢弃迟到事件。授权由桥接保存独立应答句柄，前端回传决定时直接唤醒等待中的工作线程；关闭或断线将未回答请求解析为拒绝。
4. **历史**：`Prompt` 在新增原始事件时同步保留 GUI feature 下的展示事件，展示记录与压缩后的模型上下文分开；`clear_pending_events` 不清理展示记录。`SessionRuntime::load` 已验证数据库事件链，将其交给投影重建旧会话。展示角色为用户、助手、工具及系统标记；`rollback` 标记失败轮次，不把临时流伪装为已完成消息。`Agent` 提供结构化列表、历史和未保存状态读取，终端格式化输出继续有效。无需存储迁移；存储关闭时只承诺进程内历史。
5. **前端与关闭**：React 以当前会话 ID 管理消息，检查文本区的输入法组合状态以避免候选确认时误发。Markdown 不启用原始 HTML 或远程图片，链接限定 HTTP/HTTPS；CSP 限制脚本与网络。代码块复制使用 Tauri Clipboard 插件，仅授予写文本权限。工具授权显示现有提示，默认拒绝；原授权缓存仍由 Agent 在会话切换后清空。窗口关闭事件先阻止立即销毁，解除待授权；工作线程当前轮次完成后调用保存。若存在未保存会话，呈现重试、返回或明确退出；仅内存且有对话的会话在关闭前明确提示无法保存。

## Risks / Trade-offs

- [GUI 构建需原生 Linux 依赖，当前环境缺失] → 默认构建保持可检验；GUI feature 与原生交互在装好依赖的 Linux/Windows 主机验收，缺失项清楚记录。
- [GTK/WebKitGTK 在特定 Wayland 显卡有空白或模糊窗口] → 先用原生 Wayland 与系统装饰验收，仅对实际复现的机器选用官方 workaround。
- [显示记录与压缩上下文分离可能显示更长历史] → 消息区按需加载较早条目，模型上下文预算不变。
- [等待用户授权时后台循环阻塞] → 授权应答和窗口关闭走独立句柄，不入同一个串行命令队列。

## Migration Plan

不变更数据库 schema 或旧会话格式。取消 `gui` feature 即退回现有 TUI/REPL；新环境变量未设置时保留默认行为。旧存档从原始事件链重建展示历史。
