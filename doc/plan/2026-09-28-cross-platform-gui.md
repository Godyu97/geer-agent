# Linux Wayland 与 Windows 11 GUI 实施方案

## 目标与选型

在现有 `geer-agent` 二进制上增量增加桌面 GUI。默认 `cargo run` 保持 TUI/管道 REPL 行为；构建时显式启用 Cargo `gui` feature 后，由 `.env` 的 `GEER_AGENT_UI=gui` 选择窗口界面。首版从项目目录的终端启动，使用现有模型、工具、授权和会话存储，不增加安装包、目录选择器、配置编辑器、文件树、停止生成或多窗口。

截至 2026-09-28，调研了 Rust 社区常见候选。GitHub 关注度只反映社区规模，不等于本机兼容性：

| 框架 | 调研时 GitHub Stars（约） | 平台路线 | 取舍 |
| --- | ---: | --- | --- |
| [Tauri 2.12](https://github.com/tauri-apps/tauri/releases/tag/tauri-v2.12.0) | 111.5k | Rust 后端 + 系统 WebView；Linux 为 GTK/WebKitGTK，Windows 为 WebView2 | 选用：符合已确定的 React/TypeScript 界面路线 |
| [Dioxus 0.7](https://dioxuslabs.com/learn/0.7/guides/platforms/desktop/) | 39.3k | Rust/RSX + 系统 WebView | 更适合界面也使用 Rust 的项目 |
| [Iced 0.14](https://github.com/iced-rs/iced/releases/tag/0.14.0) | 31.6k | 原生 Rust GUI，默认启用 Wayland 与 X11 | 不使用 Web 前端，聊天富文本需更多控件工作 |
| [egui/eframe 0.36](https://github.com/emilk/egui) | 30.7k | 原生即时模式 GUI，包含 Wayland 集成 | 适合工具面板与原型，本项目已选 Web 路线 |
| [Slint 1.18](https://github.com/slint-ui/slint) | 24.0k | 原生声明式 GUI，支持 Windows、Linux | 需引入 `.slint` 界面语言 |

Tauri/Wry 的 GTK 接入支持原生 Wayland，实际稳定性仍取决于 WebKitGTK、显示服务器与显卡驱动。[Wry 平台说明](https://github.com/tauri-apps/wry#platform-considerations)；[Tauri 平台依赖](https://tauri.app/start/prerequisites/)；[Linux 图形问题](https://tauri.app/develop/debug/linux-graphics/)。使用系统装饰、不透明窗口，避免首版引入额外窗口绘制变量。只有在真机重现图形问题时才使用官方文档中的环境变量 workaround，不在所有 Linux 用户上全局强制禁用加速。

## 入口、配置与依赖

唯一入口仍是 `src/main.rs`，启动时沿用统一配置目录选择逻辑加载 `.env`，再解析 `GEER_AGENT_UI=auto|gui|tui|repl`。进程环境变量优先；外部 `.env` 依次查找可执行文件同级目录、本地 Cargo 项目目录、`~/.geer-agent/`，不读取其他启动工作目录的 `.env`；`embed-env` 行为保持现状。默认 `auto` 在标准输入输出均为终端时选择 TUI，否则选择文本 REPL；`gui` 未编入时明确提示使用 `--features gui`；`tui` 在非交互终端明确报错。显式 `gui` 即使由终端或管道启动也优先进入 GUI。默认 SQLite 数据库继续落在选定配置目录的 `.db/`。

Cargo `gui` feature 仅控制编译：将 Tauri、构建辅助和 GUI 所用插件设为可选依赖。默认构建不需要 WebKitGTK、Node 或前端产物。启用 GUI 时，先构建 React/Vite 静态资源，再执行 `cargo run --features gui`；缺少资源时提供可操作的构建提示。所有 GUI Rust 代码和配置归入 `src/ui/gui/`，React/TypeScript 源码归入其 `frontend/` 子目录；不再创建第二个 Rust 二进制或业务 crate。Windows 继续用控制台子系统，从终端启动同一个程序。

标准库不提供跨平台窗口、系统 WebView、前端组件或 Rust/WebView IPC，因此引入 Tauri。React 管理消息和表单的局部状态；Vite 负责构建静态资源；`react-markdown` 与 GFM 插件渲染模型文本，禁用原始 HTML。[Tauri Clipboard 插件](https://v2.tauri.app/plugin/clipboard/)用于跨平台写入代码块文本，只赋予写文本权限，不开放读剪贴板。npm 锁文件提交；不加前端状态管理、路由或 CSS 框架。前端没有模型密钥或数据库连接权限。配置 CSP，不加载远程脚本与图片，按 [Tauri CSP 文档](https://v2.tauri.app/security/csp/)仅放行本机 `ipc:` / `http://ipc.localhost` 通道；Markdown 链接只允许 HTTP/HTTPS。

## Rust 接入与数据流

Tauri 窗口事件循环在主线程。另开工作线程，在该线程内创建单线程 Tokio runtime 和现有 `Agent`；现有 `FnMut` 确认器及 `Rc` 相关约束不需要改成 `Send`。普通操作串行进入该工作线程，通过 Tauri command 发送请求，用有序 Channel 把流式增量、用量、状态、授权请求、完整操作结果返回 WebView。授权应答走独立通道，可以在工作线程等用户选择时及时解除等待。连接断开或窗口关闭默认拒绝待授权操作；迟到应答不能用于下一条请求。执行中允许编辑草稿、滚动与复制；新提交、切换会话和压缩等待当前轮次结束。

`Agent` 增加供 GUI 使用的结构化会话列表、展示记录与未保存状态读取，原有终端格式化输出仍由 TUI/REPL 使用。IPC 增量和完成快照带操作 ID，快照中的状态带会话 ID。`Prompt` 仅在 GUI feature 下保留独立于模型上下文的完整展示记录；记录来自已有 `user`、`assistant`、`tool_step`、`compaction` 和 `rollback` 事件。上下文压缩不删展示记录，清空待保存事件也不删。跨进程打开会话时，使用现有经过校验的数据库事件链重建展示记录；无需表结构或快照迁移。未启用持久化时仍能在当前进程的会话之间展示完整记录；跨进程仅承诺最后成功发布的检查点。错误轮次及中断工具状态显式呈现，不把临时流误标为成功完成的记录。

GUI 使用会话侧栏、消息区、多行输入及用量面板。支持 Markdown/GFM、代码块复制、长记录滚动、流式更新、`/help` 等已有命令。Enter 发送、Shift+Enter 换行，中文输入法候选组合时的 Enter 只确认候选；查看旧消息时不强制滚到底部。模型、会话、上下文估算、本轮与累计真实用量、用量缺失状态与当前 TUI 保持一致。工具授权明确显示命令或路径范围，“本会话允许此工具”沿用现有会话内授权缓存，切换会话和 `/reset` 清空；“拒绝”为默认选项。

关闭窗口时先解除待授权；执行中的模型轮次完成后补写会话并关闭，保存失败则显示未保存 ID 和重试、返回或明确退出的选择。仅内存且包含对话的会话关闭前也提示无法保存，避免把持久化关闭或数据库降级误认为保存成功。GUI 中展示启动配置错误、数据库降级和运行诊断；不依赖看得见的 stderr。

## 顺序与验收

1. 用 OpenSpec CLI 新建 `add-desktop-gui`，完成中文 proposal、`gui` delta spec、design、tasks，并通过严格校验。本文件先于业务代码落盘。
2. 增加可选依赖、单入口界面分派与资源构建；验证默认 `cargo build` / `cargo run` 和未编入 GUI 时的报错。
3. 增加工作线程桥接、授权应答及完整历史投影；用两种协议的模拟流测试会话隔离、压缩前后历史、旧事件链恢复、授权拒绝、迟到消息与关闭补写。
4. 完成 React 界面及前端测试；覆盖输入法组合、Markdown 安全、长消息、流式合并、会话切换和授权。构建前端静态资源并验证 GUI feature 的 Rust 编译。
5. 运行前端类型检查、单元测试及生产构建，Rust 按 `cargo fmt --all`、相关与完整 `cargo test`、GUI feature 测试、`cargo clippy --all-targets --all-features` 顺序检查，并验证默认 TUI/管道 REPL。
6. 原生验收：Fedora/KDE 用 `GDK_BACKEND=wayland`，Windows 11 x64 用 MSVC 和 WebView2；检查中文输入、剪贴板、窗口缩放、100%–200% DPI、多显示器、流式对话、会话恢复、保存退出，以及带中文和空格路径。Windows 本地工具继续要求 Git for Windows Bash，并确认 `ls`、`rg` 可用。

当前执行环境可连接本机 Wayland socket，但未装 D-Bus、GTK 与 WebKitGTK 开发依赖，也没有 Windows 11 环境。实现记录须分别列出静态检查、模拟服务、浏览器检查和原生真机结果；未实测的平台不可写成已验证。

## 落地记录（2026-09-29）

实现位于 `/home/lihongyu/projects/geer-agent`。GUI 仍由同一个 `geer-agent` 入口启动，源码在 `src/ui/gui/`；先有本方案和 OpenSpec 产物，随后完成代码。合并时保留了项目中已有的 `.env` / 默认数据库同目录改动，GUI 直接复用该配置查找顺序。

- `cargo fmt --all`、`cargo build`、`cargo test`、`cargo clippy --all-targets` 通过。Rust 单元测试覆盖两种模型协议的展示记录、压缩、会话恢复、命令流、授权拒绝与退出保存判定；集成测试覆盖所选 `.env` 中 `GEER_AGENT_UI=gui`、进程变量覆盖、既有 REPL/模型/工具路径。测试中的临时可执行文件偶尔在复制后触发 `ETXTBSY`，因此增加了仅针对该错误的短暂重试，随后完整测试通过。
- 在 `src/ui/gui/frontend/` 执行 `npm ci`、`npm run build`（含 TypeScript 检查）和 `npm test` 均通过；前端 9 个测试覆盖中文输入法 Enter、流式操作 ID、授权拒绝、长历史、安全 Markdown 与关闭保存提示。
- `cargo check --features gui --target x86_64-pc-windows-gnu --tests`、`cargo clippy --all-targets --all-features --target x86_64-pc-windows-gnu` 通过，验证 GUI Rust 代码可在已安装的 Windows GNU 交叉目标上编译检查；这不是 Win11 MSVC 构建或运行测试。
- `openspec validate add-desktop-gui --strict` 通过。默认二进制的管道 `/exit` 正常；未编入 GUI 而选择 `GEER_AGENT_UI=gui` 时明确报错，非交互环境强制 TUI 也明确报错。
- 本机 `wayland-info` 能连接 Wayland socket，但 `cargo check --features gui` 停在 `libdbus-sys`：缺少 `dbus-1.pc`；`pkg-config` 同时报告缺少 GTK 3 与 WebKitGTK 4.1 开发包。因此尚未打开 Linux 原生窗口，也没有实测 Wayland 输入法、剪贴板、窗口缩放或多显示器。当前没有 Windows 11 环境；Win11 MSVC/WebView2 真机验收同样待完成。

## 落地补充（2026-10-02）

- Windows 11 MSVC：release 桌面程序 `geer-agent-desktop.exe` 启动成功，PE Subsystem=2，窗口标题 `geer-agent`，无额外控制台；通用程序 Subsystem=3。同日原生 Make 双产物与哈希已核对。
- 本轮未在 GUI 内手工完成输入法、剪贴板、DPI/多显示器与会话保存点击路径；Fedora Wayland 原生验收仍未执行。
