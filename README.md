# geer-agent

GeekAgent 教程的 Rust 学习实现。

## 配置与运行

普通 `cargo run` 可使用项目根目录的 `.env`：复制 `.env.example` 为 `.env`，填写 `OPENAI_API_KEY` 和 `OPENAI_MODEL`，然后运行：

```sh
cargo run
```

复制可执行文件到其他目录运行时，优先读取可执行文件同级的 `.env`；若不存在，则读取 `~/.geer-agent/.env`。普通本地 Cargo 构建产物在同级没有 `.env` 时，也会检查项目根目录的 `.env`。启动工作目录中的其他 `.env` 不参与查找。所有候选位置都没有外部 `.env` 时，也可以只设置同名进程环境变量。`OPENAI_BASE_URL` 可选，默认使用 OpenAI 地址；`OPENAI_API` 可选，默认使用 Responses API，另可设为 `chat-completions`。

`GEER_AGENT_UI=auto|gui|web|tui|repl` 可写在上述位置的 `.env`，进程环境变量优先。默认 `auto` 在交互终端使用 TUI，在输入或输出接管道时使用文本 REPL；显式 `tui` 要求交互终端。`gui` / `web` 分别要求 Cargo 的 `gui` / `web` feature，否则程序会提示构建方式。四种界面继续使用同一个 `geer-agent` 可执行文件。

## 统一构建与界面选择

日常使用三个 Make 入口，均包含 GUI、Web、TUI 和 REPL，自动安装前端依赖并构建两种静态页面：

| 命令 | Cargo 调用 | 结果 |
| --- | --- | --- |
| `make build` | `cargo build --features gui,web` | 仅构建 debug，产物在 `target/debug/` |
| `make release` | `cargo build --release --features gui,web` | 仅构建 release，产物在 `target/release/` |
| `make run` | `cargo run --features gui,web` | 构建 debug 并启动 |

Cargo 没有 `cargo release` 命令，release 构建使用 `cargo build --release`。两种 profile 的通用程序都名为 `geer-agent`（Windows 为 `geer-agent.exe`），支持同样的四种界面。

Windows 下，`make build` / `make release` 还会自动编译 `desktop-gui`，在 `target/debug/` / `target/release/` 同时交付 `geer-agent.exe` 和 `geer-agent-desktop.exe`，无需额外命令或参数。桌面版默认打开 GUI，只支持 GUI，双击时不创建额外控制台。桌面版使用独立编译缓存 `target/desktop-gui/`，不会覆盖通用程序；首次构建耗时与缓存占用会增加。设置 `CARGO_TARGET_DIR` 时，上述产物与缓存均跟随该根目录。以 Make 成功退出为构建成功依据，失败时可能仍保留上一轮的产物。Windows 原生 GNU Make 使用系统 `cmd.exe` 执行配方，复制与清理使用内建命令，无需把 Git Bash 的 `sh`、`cp`、`rm` 加入 PATH；程序运行时的 Bash 工具仍需要 Git for Windows。Linux 等非 Windows 环境仍只生成通用程序，`make run` 仍只构建并启动通用程序。

**通用程序的启动界面由 ENV 决定，构建时不固定 UI。** 可以在程序选用的 `.env` 中设置 `GEER_AGENT_UI=gui|web|tui|repl|auto`；进程环境优先，同一个通用产物切换 UI 无需重新编译。

```sh
GEER_AGENT_UI=gui make run
GEER_AGENT_UI=web make run
GEER_AGENT_UI=tui ./target/release/geer-agent
GEER_AGENT_UI=repl ./target/release/geer-agent
```

Windows PowerShell：

```powershell
$env:GEER_AGENT_UI = 'gui' # 可换为 web、tui、repl 或 auto
.\target\release\geer-agent.exe
```

通用构建需要 Bun、Node 和下文的 GUI 原生依赖。仅需终端时直接 `cargo build` / `cargo run`，无需前端或 GUI 原生依赖；仅需 Web 时先 `make frontend-build`，再 `cargo build --features web`。精简构建若选择未编入的 UI，会明确报错。

## 桌面 GUI（可选）

GUI 使用 Tauri 2 + React/TypeScript，与 Web 共用前端。开发需要 Bun 1.4.2 和兼容 Vite/tsc/Vitest 的 Node.js；依赖和脚本入口统一使用 Bun。先构建静态前端，再启用 Rust feature：

```sh
cd src/ui/frontend
bun install --frozen-lockfile --concurrent-scripts 1
bun run build:gui
cd ../../..
cargo run --features gui
```

在程序实际选用的 `.env` 中设置 `GEER_AGENT_UI=gui` 后，上述 `cargo run` 会打开窗口。未设置时仍按 `auto` 选择终端界面；也可以执行 `GEER_AGENT_UI=gui cargo run --features gui` 临时启动。GUI 提供完整会话记录、侧栏会话切换、Markdown 回答、流式输出、用量与工具授权。现有 `/help`、`/new`、`/open`、`/save`、`/compact`、`/sessions`、`/exit` 等命令可直接在输入框使用。Enter 发送，Shift+Enter 换行；授权默认拒绝。关闭窗口时等待正在执行的请求并补写会话，保存失败时可重试、返回或明确退出。若持久化已关闭或数据库不可用，关闭含有对话的仅内存会话前会提示数据无法保存。

通用程序通过 `GEER_AGENT_UI=gui` 启动桌面窗口，并保留 TUI/REPL 和 Web 选择。Windows 通用程序保留控制台子系统，从已有终端运行时终端会保留；双击通用程序可能出现控制台窗口。

如需 Windows 双击完全无控制台，执行 `make build` 或 `make release`，再打开对应目录的 `geer-agent-desktop.exe`。它默认进入 GUI，仅接受 `GEER_AGENT_UI=auto` 或 `gui`；配置错误在窗口显示，后台工具执行不弹出控制台。仍可在 `make frontend-build` 后直接执行 `cargo build --release --features desktop-gui`，但这条直接 Cargo 命令会替换 `target/release/geer-agent.exe`，切回通用程序需重新 `make release`。

Windows 手工验收（在安装原生构建依赖的 Windows 环境执行）：

1. 分别运行 `make build` 和 `make release`，确认 `target/debug/`、`target/release/` 各有两份 exe；若设置了 `CARGO_TARGET_DIR`，检查对应目录。
2. 在 Visual Studio 开发者终端用 `dumpbin /headers` 检查两种 profile 的两份 exe：通用版 Subsystem 应为 Windows CUI，桌面版应为 Windows GUI。
3. 配置有效的模型参数，将 `GEER_AGENT_UI` 留空或设为 `auto`，分别双击 debug/release 的 `geer-agent-desktop.exe`，确认默认打开 GUI 且无额外控制台；在 GUI 中授权执行一个有限命令，确认后台工具也不弹窗。
4. 临时将 `GEER_AGENT_UI` 设为无效值，再启动桌面版，确认窗口显示配置错误；验收后恢复原配置。
5. 从 PowerShell 启动通用版，先设 `$env:GEER_AGENT_UI = 'tui'` 验证终端界面，再设为 `gui` 验证窗口界面；完成后恢复原进程环境。两种 profile 都应保持通用入口的模式选择能力。

Linux 桌面入口模板位于 `src/ui/gui/geer-agent.desktop`：把 `Exec` 改为桌面可执行文件的实际绝对路径，保留双引号及 `Terminal=false`，然后保存到 `~/.local/share/applications/geer-agent.desktop`，从应用菜单打开。启动器直接运行二进制，不调用 Cargo 或终端模拟器。配置仍按既有 `.env` 查找顺序加载，无需以项目目录作为启动工作目录。原因、技术方案及验收步骤见 [GUI 启动优化方案](doc/plan/2026-10-01-gui-launch.md)。

Linux 需要 WebKitGTK 4.1 等 Tauri 开发依赖。Fedora 按 [Tauri 官方前置要求](https://tauri.app/start/prerequisites/) 安装 `webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel` 及 C 开发工具；如编译提示找不到 `dbus-1.pc`，还需 `dbus-devel`。从 Wayland 会话中运行，可用 `GDK_BACKEND=wayland` 做原生 Wayland 验收。Windows 11 使用 MSVC Rust 工具链、Microsoft C++ Build Tools、WebView2 、Bun 1.4.2 和兼容 Vite 的 Node.js；本项目工具执行还需要 Git for Windows Bash。前端产物嵌入启用 GUI 的二进制，修改前端后需重新运行 `bun run build:gui` 和 Cargo 构建。默认 `cargo build` 不需要 Node 或 GTK/WebKitGTK。

侧栏的“管理会话”支持勾选、全选当前列表和批量删除；切换“当前 / 全部”范围会清空选择。确认弹窗显示标题、完整 UUID 和当前会话替换提示，默认取消。删除其他会话或取消操作会保留聊天草稿；删除当前会话成功后，消息区和草稿清空。失败项可点击“重试删除 / 清理”。

## 浏览器 Web UI（可选）

```sh
GEER_AGENT_UI=web make run
# 修改监听端口；默认 8827
GEER_AGENT_UI=web GEER_AGENT_WEB_PORT=8828 make run
# 构建通用 release 程序，再以 Web 模式启动
make release
GEER_AGENT_UI=web ./target/release/geer-agent
```

`GEER_AGENT_UI=web` 选择独立 Web 宿主；监听固定为 `0.0.0.0`，端口由 `GEER_AGENT_WEB_PORT` 配置，范围 1–65535。打开启动输出中的本机地址，或从手机访问服务器的局域网 IP。端口非法、被占用、缺少模型配置会明确退出。仅启用 web 的精简构建不依赖 GTK/WebKitGTK；Make 通用构建包含 GUI，因此需要 GUI 原生依赖。默认 Cargo 构建不需要前端。

未设置或留空 `GEER_AGENT_WEB_TOKEN` 时，启动会生成并显示一次临时访问口令；在 `.env` 或进程环境中设置非空值可固定口令，固定口令不打印。登录凭证存于本进程内存，通过 HttpOnly/SameSite Cookie 传递，重启后需要重新登录。写请求和 WebSocket 校验同源；模型密钥留在 Rust 服务端。

这是单用户共享会话：多个浏览器共用当前会话和 workspace，操作互斥，状态过期的提交会被拒绝。重连恢复完整历史、正在生成的回答和授权，未确认的操作不会自动重发。各页面的输入草稿按会话 UUID 保留。任一已登录端的第一份工具授权回复生效；无人回复 120 秒或所有端断开后默认拒绝。workspace 输入的是**服务器目录**，工具也在服务器执行。GUI 与 Web 是独立运行模式。

Web 保留 GUI 的聊天、Markdown、完整历史、工具进度、用量、压缩、保存和会话管理功能，以及 Catppuccin Mocha 视觉。手机通过顶部会话和状态按钮展开面板；局域网 HTTP 无法自动复制时提供可选择的代码。输入 `/exit` 保存后只离开当前页面，关闭标签页也不会停止服务；终端 Ctrl+C/SIGTERM 才停止服务并保存，保存失败返回非零状态。

服务最多接收 16 个 WebSocket，每端待发送事件上限 256；慢端断开后可重新同步。输入上限 65536 字节，入站帧/消息上限 512 KiB；15 秒探测一次，45 秒无响应断开。

## 共用 React / TypeScript 的管理方式

唯一前端包位于 `src/ui/frontend`，只维护 `bun.lock`。依赖升级在这里进行，安装必须使用 `bun install --frozen-lockfile --concurrent-scripts 1`；构建和测试使用 `bun run`，Vite、tsc、Vitest 沿用兼容 Node 的运行方式。不要为 Web 再复制 App、会话管理或样式。

- `protocol.ts` 统一事件/快照类型，`model.ts` 管理纯状态更新；新业务行为在共用层实现和回归。
- `App.tsx` 与 `style.css` 共用交互和视觉，宿主差异通过 `HostAdapter` 注入；组件只调用适配接口。
- `hosts/desktop.ts` 处理 Tauri IPC、目录选择和系统剪贴板，`hosts/web.ts` 处理 WS、重连和浏览器剪贴板；Web 登录在 `WebGate.tsx`。
- Vite 模式在构建时选择宿主，分别输出 `dist/gui` 和 `dist/web`；Web 产物不引入 Tauri IPC。继续拆组件时按消息、会话面板、授权弹窗等能力拆进本包，无需建立两个应用或发布 UI 库。

```sh
make frontend-check
make frontend-test                 # 受限 Vitest，共用 GUI/Web 回归
make frontend-build                # bun run build:gui，再 build:web
make web-test                      # 前端 + 受限 Rust Web 测试
make clippy-all                    # 两种前端 + gui,web Rust 检查
```

Make 不维护旧 GUI/Web 专用构建和运行别名。`BUN=/absolute/path/to/bun` 可覆盖 Make 的 Bun 路径。UI 接入逻辑和维护边界见 [架构文档](docs/design/architecture.md)。

## 终端界面

在交互终端运行 `cargo run` 会进入全屏 TUI：左侧是可滚动的消息区，底部输入，宽度至少 60 列时右侧显示模型、会话、上下文估算与 token 用量。终端变窄时会隐藏面板。方向键编辑输入，PageUp/PageDown 或上下方向键滚动消息；`/help`、`/new`、`/sessions`、`/open`、`/compact` 等命令沿用原语义。工具首次授权会在界面内要求 `[y/N]`，只有输入 `y` 或 `yes` 才会同意。

面板的上下文 token 与占比是估算值；流式回答期间用 `~` 标记临时用量，完成后显示接口报告的真实 token 数。本轮统计包含该条输入触发的模型和自动压缩请求，累计统计涵盖本进程中的会话及手动压缩；若兼容接口未提供某次用量，面板显示“用量部分缺失”，估算值不会计作真实用量。`/exit`、Ctrl+C、Ctrl+D 会尝试保存会话并恢复原终端；全屏期间暂存的诊断记录在退出后写到 stderr。输入或输出接管道时继续使用原文本 REPL。

F3 或 `/sessions [--all]` 打开会话管理面板并保留聊天草稿。上下方向键、PageUp/PageDown 移动，Space 勾选，`a` 全选/取消全选，Delete 请求删除，Enter 打开高亮会话，Tab 切换范围，Esc 返回聊天。删除确认时 Enter/Esc 取消，`y` 删除；结果页可滚动，`r` 重试失败项，Enter/Esc 返回列表。

## 工具调用

四种界面向模型提供 11 个工具：`get_current_time`、`bash`、`ls`、`glob`、`rg`、`read`、`write`、`edit`、`search`、`web_search`、`web_fetch`。模型选择工具后，程序执行并把结果交回模型继续回答。`GEER_AGENT_TOOLS=off` 可在启动时关闭所有工具；默认开启。服务端模型需要支持所选 API 的函数工具调用协议。

`read` 接受 `path`、可选的 1 起始行号 `offset` 和行数 `limit`，单次最多返回 2000 行、50 KiB 的 UTF-8 文本。`write` 接受 `path` 与 `content`，创建或覆盖文件。`edit` 接受 `path` 和 `edits` 数组，其中每项是 `oldText`、`newText`；旧文本必须在原文件中唯一匹配，各项不能重叠。`bash` 接受 `command`，在当前 workspace 运行，10 秒超时，结果最多 2000 字符。

`GEER_AGENT_BASH_BIN` 可指定 Bash 可执行文件的绝对路径。未设置或为空时，Windows 优先使用 `C:\Program Files\Git\bin\bash.exe`，该文件不存在时再退回 `PATH` 中的 `bash`；Linux 等其它系统直接使用 `PATH` 中的 `bash`。这样可以避开 Windows System32 里的 WSL `bash.exe`。工具会自动把 Git 自带的 `usr\bin` 加入命令的 `PATH`，并把 workspace 以 `F:/repo` 形式传给 Bash。启动时会验证所选 Bash 并读取版本，路径或版本无效时直接报错退出。每次模型请求都会带默认系统提示，其中 `<context_data>` 包含系统版本和所选 Bash 版本；`/reset` 后仍会提供这些环境信息。

Bash、目录查询、文件工具和 `search` 各自在当前会话首次使用时请求 `y/N` 授权。每次切换会话（包括切回旧会话）都会清空授权；打开失败或打开当前会话则保持授权。非交互输入无法确认时默认拒绝执行。

`ls` 列目录，`glob` 找路径，`rg` 搜正文，固定查询都通过配置的 Bash 执行，需要其 `PATH` 中有 `ls` / `rg`。这些已有工具保留任意路径授权范围。新增 `search` 只搜索当前 workspace：默认 `path="."`、`output="content"`、区分大小写的正则，可选 `glob`、`fixed_strings`、`ignore_case`；`output="files"` 只返回命中文件。结果为 `相对 workspace 路径:行号: 原文`，保留缩进，子目录结果也能直接交给 `read`。规范化后越界的路径与链接会被拒绝，不跟随目录链接；沿用 rg 忽略规则，显式路径或 glob 可覆盖默认过滤，跳过二进制及超过 1 MiB 的文件。最多 50 项、总输出 2000 字符，截断时只保留完整命中行并提示缩小范围。

`web_search` 接受 `query` 和可选 `num_results`（默认 5，范围 1–20），通过 [Exa 免 Key MCP](https://exa.ai/docs/get-started/exa-mcp) 返回标题、完整 URL、摘要和后端名称。首次会话授权会明确说明查询将发送给 Exa；服务的 HTTP、RPC 或超时错误会如实报告。`web_fetch` 接受完整 HTTP(S) `url`，直接抓取 HTML、Markdown、JSON 等文本，返回来源与最终 URL、响应信息及可读正文；HTML 清除脚本、样式和标签，保留段落、列表与代码缩进。支持公网、本机和内网地址，每次访问都显示完整 URL 并确认，跨来源重定向再次确认，最多跟随 5 次。拒绝后停止访问目标。

网页搜索网络超时 25 秒，抓取网络总耗时 15 秒（用户确认等待不计入），响应体上限 1 MiB，单次结果上限 12000 字符。客户端复用 HTTP/TLS 与环境代理配置，不携带模型 API key、Cookie 或用户认证头。网页摘要和正文作为不可信数据交给模型。首版只提供静态文本访问；PDF、JavaScript 渲染、缓存及抓取回退暂未实现。

## 项目指令与长期记忆

项目指令只读取当前 workspace 根目录的 `AGENTS.md`，不读取父目录、子目录或用户目录中的规则，也不读取 `CLAUDE.md`。启动、切换 workspace、打开会话和每条用户消息前重新读取全文；同一条消息的工具循环使用同一份指令，修改在下一条消息生效。根文件缺失或为空时正常运行并显示无指令；其他读取错误会明确报告，失败的切换保留原会话。指令独立于历史和摘要，恢复旧会话使用当前文件，压缩后仍完整提供给模型。

长期记忆默认启用，保存在公共数据库的独立 `agent_memories` 表（MongoDB 为同名集合），同一数据库下所有 workspace 和会话共享。每条记录包含完整 UUID、正文、创建和更新时间。正文去除首尾空白，重复添加返回已有记录；编辑保留 UUID 和创建时间，不能覆盖成另一条已有正文。数据库写入成功才显示成功，不回退为临时记忆。

Agent 自动执行 `memory_write(content)` 保存用户偏好、项目事实和重要决定，使用 `memory_search(query)` 按需回忆，不会将全部记忆自动注入提示。搜索按空白拆词、不区分大小写，命中任意词即可；命中词数越多越靠前，同分按创建时间和 UUID 排序，最多十条。工具结果最多 2000 字符，界面可以查看全文。

每条用户消息前还会自动召回相关记忆：中文问题按相邻双字拆分，英文和数字按词匹配；长记忆以 320 字符窗口、80 字符重叠检索，BM25-lite 排序后最多选择五个不重叠片段。每段附完整 UUID、段号、原文字符范围与更新时间，整份参考消息的估算用量不超过上下文窗口的八分之一、且最多 2048 Token；放不下完整片段时略过。查询最多处理前 4096 字符、64 个不同有效词。这是字面检索，不支持同义词理解；明确关键词的手动搜索仍保留上述规则。

召回作为本轮参考资料提供给模型，不能覆盖当前请求、项目规则或工具授权；两种 API、四种界面共享同一条路径。同一问题的工具续轮使用同一份召回，下个问题重新读库，编辑或删除在下轮生效。召回本身不进入会话检查点、原始聊天事件或压缩输入，但模型引用后的回答、手动工具结果和已有 Trace 仍保留。实现与 pi、Codex、教程的对比见 [记忆与检索设计](docs/design/memory-retrieval.md)。

可用 `/memory add 历史压缩保留最近消息，以便继续当前任务` 保存一条事实，再直接询问“为什么历史压缩要保留最近消息”，相关片段会在首次模型请求前准备好，无需模型先调用搜索工具。

四种界面都可直接管理记忆，不触发模型调用：TUI 按 F4 或输入 `/memory`，GUI/Web 点击顶部“记忆”。TUI 中 `/` 搜索、`a` 新增、`e` 编辑、Enter 查看全文、Delete 删除、`c` 清空，Esc 返回；编辑支持粘贴多行与 Shift+Enter 换行，失败时保留正文。图形面板支持窄屏、全文、多行编辑和搜索，管理操作保留聊天草稿。状态面板显示根指令是否加载、记忆状态及总数。

| 公共命令 | 功能 |
| --- | --- |
| `/memory` | 查看全部全局记忆 |
| `/memory search Rust 中文` | 按关键词搜索 |
| `/memory add 回答使用 Rust 示例` | 添加正文 |
| `/memory edit <完整 UUID> <正文>` | 编辑指定记忆 |
| `/memory delete [--yes] <完整 UUID>` | 删除指定记忆 |
| `/memory clear [--yes]` | 清空全部长期记忆 |

删除与清空默认取消。管道输入必须带 `--yes`，未确认的命令不会消耗后续输入；GUI/Web 即使手动输入 `--yes` 也需要先预览并确认，确认只属于发起端，目标或状态改变、断线后需要重新预览。清空影响所有 workspace 的长期记忆，保留会话、Trace、项目指令和历史中已经引用的内容；空库清空和重复删除可以安全重试。

`GEER_AGENT_MEMORY=off` 独立关闭记忆，保留已存数据；`GEER_AGENT_MEMORY_RECALL=off` 只关闭自动召回（默认 `on`），仍能管理和手动搜索记忆。`GEER_AGENT_TOOLS=off` 只关闭模型工具，UI 管理和自动召回仍可使用。关闭会话持久化与 Trace 后，记忆仍可跨重启保存；初始化或读取失败会显示不可用，普通聊天可继续，记忆操作会报告错误。运行中读取失败不复用旧召回，数据库恢复后下一轮可再次读取。

## 上下文压缩与会话恢复

`Prompt` 保留当前系统环境提示、历史摘要和近期原文。每次模型请求前估算上下文用量；默认窗口为 272,000 tokens，到 90% 时自动请求同一模型生成摘要。摘要保留任务目标、约束、进展和待办，并以普通历史背景交给模型；完整工具调用与结果不会在中间切开。估算依据消息的序列化体积和服务端报告的实际输入用量，可能与所用模型的 tokenizer 不同。

| 配置 | 默认 | 用途 |
| --- | --- | --- |
| `GEER_AGENT_CONTEXT_WINDOW_TOKENS` | `272000` | 当前模型的上下文窗口；请按实际模型调整 |
| `GEER_AGENT_AUTO_COMPACT` | `on` | 设为 `off` 时仅保留手动 `/compact`；明确的上下文溢出仍可压缩并重试一次 |
| `GEER_AGENT_SESSION_PERSISTENCE` | `on` | 设为 `off` 时只保留进程内多会话，不写会话检查点 |

`/new` 新建会话；`/open <session-id>` 打开进程内会话或当前目录存档；`/reset` 等同于 `/new`，`/resume <session-id>` 等同于 `/open`。`/sessions` 合并当前进程会话与最近 20 条存档，以 `*` 标记当前会话并显示保存状态。`/save` 重试所有待写会话。`/exit` 和 EOF 也会补写，并列出仍未保存的 UUID。启动时总是新建会话，历史会话需显式打开。`/help` 显示完整命令。

会话标题从第一条用户输入生成：合并空白、移除控制字符，取前 20 个 Unicode 字符，超长追加省略号，空会话显示“新会话”。标题只用于展示，UUID 仍是唯一标识，数据库和快照不增加标题字段；压缩和重新打开会话后仍从原始消息取标题。

`/delete <完整 UUID> [UUID ...]` 删除一个或多个会话，交互终端会先确认，默认取消。管道或脚本须使用 `/delete --yes <完整 UUID> [UUID ...]`；不接受标题、短 UUID 或通配符。删除会话快照及其全部消息事件，保留 Trace 日志，逐项报告成功或失败。删除当前会话成功后，在原 workspace 创建空会话并清空工具授权与本轮用量，累计用量保持。全选仅覆盖界面已列出的会话，未加载存档不会被删除。MongoDB 可能报告快照已删除而消息清理待重试，此时再次删除相同 UUID 即可继续清理。

每条会话独立持有消息、摘要、上下文估算偏差及待写事件。数据库初始化失败时会告警并继续使用进程内多会话；写入失败时保留本地待写数据，切换会话也不会丢弃它。revision 冲突会显示在列表或保存结果中，程序不会自动覆盖数据库记录。跨进程只能恢复最后一次成功发布的检查点。同一工作目录中的文件仍由各会话共享。

会话存储将脱敏后的用户输入、模型输出、工具结果和摘要事件追加保存，并发布可恢复的检查点；压缩不会删除原始事件。恢复要求相同工作目录、模型、API 类型和端点，重新生成系统环境提示并清空工具授权。工具执行中断后会提示副作用未确认，不自动重跑工具。已知 API key、数据库 URL 和认证头在保存前脱敏；会话记录仍可能含其他敏感业务内容，请保护数据库及备份。本版不自动清理会话。

会话、Trace 和长期记忆默认共用所选配置目录的 `.db/geer.sqlite`，目录会自动创建：普通 `cargo run` 使用项目根目录时写入项目的 `.db/`；可执行文件旁有 `.env` 时写入其同级 `.db/`；使用 `~/.geer-agent/.env`，或仅使用进程变量、内嵌配置时写入 `~/.geer-agent/.db/`。项目根目录的 `.db/` 已加入 Git 忽略。可用公共配置改用其他数据库或指定旧文件：

```sh
GEER_AGENT_DATABASE_URL='sqlite:///absolute/path/to/previous/.db/geer.sqlite?mode=rwc'
```

`GEER_AGENT_DATABASE` 默认为 `sqlite`；选择其他后端时必须设置对应 URL。省略 `GEER_AGENT_DATABASE_URL` 才会使用上述同目录默认路径。Trace、会话和长期记忆共用这组数据库配置，可分别通过 `GEER_AGENT_TRACE`、`GEER_AGENT_SESSION_PERSISTENCE` 和 `GEER_AGENT_MEMORY` 关闭。已有数据库可以通过公共配置指定原地址，无需迁移数据。

## 执行预算

每条输入默认在第 12 个模型响应后提示收敛；若近期持续取得新结果，通用提示可推迟到第 15 个响应。最多使用 30 个模型响应、100 次实际工具调用和 10 分钟。单次响应中的多个已授权 `read` 可以并行执行；`write`、`edit`、`bash` 按调用顺序执行。连续相同结果、连续错误或连续无进展会先提示模型改变方法，仍继续时关闭工具并请求最终回答。

以下进程环境变量或 `.env` 项可选：

| 名称 | 含义 |
| --- | --- |
| `GEER_AGENT_MAX_DURATION_SECONDS` | 单条输入的总时长上限，默认 600 |
| `GEER_AGENT_MAX_INPUT_TOKENS` / `GEER_AGENT_MAX_OUTPUT_TOKENS` | 模型报告的累计输入/输出 Token 上限 |
| `GEER_AGENT_MAX_COST_USD` | 按显式单价计算的美元费用上限 |
| `GEER_AGENT_INPUT_USD_PER_MILLION_TOKENS` / `GEER_AGENT_OUTPUT_USD_PER_MILLION_TOKENS` | 每百万输入/输出 Token 的美元单价；配置费用上限时两项都必填 |

启用 Token 或费用上限后，兼容接口若不返回 Token 用量，程序会停止继续执行工具并尝试最终回答。Chat Completions 会请求流式用量尾包；兼容接口可能不支持。每次运行及工具调用的结构化指标写到 stderr，含结束原因、用量、耗时和摘要，不含工具参数、结果正文或 API key。

## LLM 调用 Trace

启动时会显示 `Session ID`，输入 `/new` 或 `/reset` 后生成并显示新 UUID。每个模型步骤（包括摘要调用）有独立的 `request_id`，同一步骤的重试合并为一条逻辑记录。Trace 默认写入上述共用 SQLite；设 `GEER_AGENT_TRACE=off` 可关闭。

数据库后端支持 `sqlite`、`postgres`、`mysql`、`mongodb`，相应 URL 使用 `sqlite:`、`postgres://` 或 `postgresql://`、`mysql://`、`mongodb://` 或 `mongodb+srv://` 协议；MongoDB URI 必须带数据库名。配置可写在 `.env` 或进程环境变量中。SQL 数据库首次连接时自动运行版本化迁移；MongoDB 自动建立索引。Reader 提供代码接口，包括按 Request ID 读取和按 Session 游标分页，本次没有终端查询命令。

启用后，所选数据库会保存每次模型调用的**完整请求、合并后的响应或失败前已收到的内容**，其中可能包含对话历史、用户输入、工具参数与工具结果。已配置的 API key 与数据库连接串即使出现在内容中也会替换为 `[REDACTED]`。请保护数据库文件、服务与备份；本版不自动清理记录。记录不含 HTTP 认证头。数据库连接或写入失败会在终端报 Trace 告警，模型对话继续。

需要将配置随单个二进制携带时，在项目根目录准备 `.env`，然后运行：

```sh
cargo build --release --features embed-env
```

构建产物位于 `target/release/geer-agent`，可复制到没有 `.env` 的目录运行。启用此 feature 时，构建目录缺少 `.env` 会导致编译失败。配置优先级为进程环境变量 > 可执行文件同级 `.env` > `~/.geer-agent/.env` > 构建时内嵌的 `.env`；只使用内嵌配置时，默认数据库位于 `~/.geer-agent/.db/`。内嵌构建不会把项目根目录的文件当作运行时外部 `.env`。默认构建不包含 `.env`。

**内嵌的 `.env` 原文可从二进制提取，其中的 API key 不是加密存储。请只向可信对象分发此产物；修改内嵌配置后需重新构建。**

## 开发命令

根目录 `Makefile` 包装了常用 Cargo / 前端命令（需要 GNU Make）：

`build`、`release`、`run`、前端构建/类型检查、`fmt`、`fmt-check`、`doc` 和 `clean` 支持 Linux 与原生 Windows。工具路径可通过 `CARGO`、`BUN` 等 Make 变量覆盖，路径带空格时将整个赋值作为一个参数传入，例如 PowerShell 中的 `make release 'CARGO=C:/Program Files/Rust/bin/cargo.exe' 'CARGO_TARGET_DIR=target with spaces'`。这些工具变量只填写可执行文件路径，不附加参数。Windows 的测试、clippy 和完整检查入口会明确提示需要 Linux 隔离环境并失败退出；其余检查约束见下节。

```sh
make help      # 列出目标
make build     # 全界面 debug 构建，不启动
make release   # 全界面 release 构建，不启动
make run       # debug 构建并启动，由 GEER_AGENT_UI 选择界面
make check     # fmt -> test-safety -> test -> clippy（默认终端配置）
```

## 测试与系统安全

2026 年 9 月的 Fedora 诊断报告确认主机发生过全局 OOM，ChatGPT/Codex 服务被内核终止。项目的缺失命令测试存在可复现的触发机制：清空 `PATH` 后，Bash 在远程启动条件下读取宿主启动文件，Fedora 的缺失命令处理再次查找缺失的 `gettext`，递归派生进程并消耗内核内存。历史证据不足以把每次 OOM 都归因于同一测试。增加 swap 无法替代修复启动隔离和限制进程数量。

在 Fedora/Linux 上，使用下面的入口运行测试：

```sh
# 修复 Shell/进程代码后，先跑受影响的单项和清理回归，再跑完整质量门。
make test-safety
make test TEST_ARGS='--bin geer-agent' TEST=query_dependencies_are_resolved_by_configured_bash_only
make test TEST_ARGS='--test process_safety'
make check

# GUI Rust 测试先准备前端产物；前端测试也走受限入口。
./scripts/test-safe.sh make frontend-build
make test FEATURES=gui,web
make clippy-all
make frontend-test
```

桌面构建的测试覆盖共用业务与模式选择；依赖 REPL 输入输出的进程集成测试仅在普通构建中运行，由 `make check` 验证。

`make test`、`make frontend-test`、`make clippy` 和 `make clippy-all` 通过 `scripts/test-safe.sh` 启动独立的 systemd 用户服务。Cargo 编译、测试程序与后代进程都归入本次服务的 cgroup；前端测试安装依赖和全界面 clippy 准备前端的阶段也先进入受限服务。入口检查内核实际的内存、swap、任务限制和临时目录挂载，缺少 systemd 用户服务、cgroup v2、有效限制或临时目录隔离时直接失败，不自动执行无约束测试。`make check` 按 fmt → test-safety → test → clippy 顺序执行，即使传入 `make -j` 也保持这个顺序。

每个服务设置 `PrivateTmp=disconnected`，使用独立 tmpfs 中的 `/tmp` 和 `/var/tmp`，并固定 `TMPDIR`、`TMP`、`TEMP` 为 `/tmp`。标准库临时文件和直接写入 `/tmp` 的测试都使用本轮私有目录，不向宿主 `/tmp` 累积文件。入口比较宿主与服务内目录的设备号，并确认文件系统为 tmpfs，实际隔离不成立就拒绝启动测试。主机需要支持此选项的 systemd 和可用的用户/挂载命名空间。

正常退出、断言失败、panic、超时、SIGKILL 或组内 OOM 后，systemd 停止整组进程并回收私有临时文件与 tmpfs；异常清理不依赖测试的 `Drop` 或 Shell 的 `trap`。若连外层启动脚本也被 SIGKILL，服务仍由运行时限兜底，最迟在运行时限加 5 秒停止等待后回收。测试过程中 tmpfs 仍占用内存，计入本轮 cgroup 内存额度，服务结束后释放；正常路径仍应及时删除不再使用的夹具，降低同轮峰值。[systemd 的 `PrivateTmp` 定义](https://github.com/systemd/systemd/blob/v259/man/systemd.exec.xml) 说明了私有临时目录的生命周期。

`make test-safety` 使用 Python 3 标准库验证正常退出、失败、SIGKILL、超时、组内 OOM、启动脚本 TERM/KILL，以及隔离预检失败。控制服务限额为 256 MiB / 64 任务 / 90 秒，各探针顺序进入 64 MiB / 32 任务 / 2 秒的独立服务；OOM 探针最多申请 96 MiB，不执行无界分配。回归检查临时文件不跨命名空间可见、调用者临时目录变量被覆盖、服务进程与 cgroup 被回收，且上一层文件不被改动。

| 限制 | 默认值 | 调整方式 |
| --- | --- | --- |
| 整个测试/编译组的内存 | 4 GiB | `GEER_TEST_MEMORY_MAX` |
| swap | 0 | 固定禁用 |
| 任务数（包括线程） | 256 | `GEER_TEST_TASKS_MAX` |
| 整个服务的运行时间 | 10 分钟 | `GEER_TEST_RUNTIME_MAX` |
| 停止后的强制清理等待 | 5 秒 | 固定；`KillMode=control-group` |
| Cargo 编译并行数 | 2 | `CARGO_BUILD_JOBS` |
| Rust 测试线程数 | 1 | `RUST_TEST_THREADS` |
| Vitest worker / 文件并行 | 1 / 关闭 | 前端测试脚本固定设置 |
| Bash 函数嵌套保护 | 32 | 固定；`FUNCNEST` 只作补充保护 |

容量需要覆盖编译峰值，GUI 构建尤其如此。运行时限接受正整数秒、`s`、`min` 或 `h`，不接受无限或零时限。遇到服务内存或任务上限失败时，先看服务名、退出原因与峰值统计，检查递归/泄漏，再逐项调整有限额度，同时为桌面和远程连接服务留出内存。例如：

```sh
GEER_TEST_MEMORY_MAX=6G GEER_TEST_RUNTIME_MAX=15min make test FEATURES=gui
```

需要其他 Cargo 参数时，用 `TEST_ARGS`；需要直接验证其它命令时，用 `./scripts/test-safe.sh <命令> [参数...]`。不要直接在开发主机运行裸 `cargo test`、`bun run test` 或测试二进制，也不要同时启动多组测试去叠加资源额度。Windows、macOS 和没有 systemd 用户服务的环境需要先提供带内存、进程数量、总时限和后代清理能力的独立测试环境；受限入口不会静默降级。不要对整个 `user.slice` 设置测试额度，否则会一起限制 ChatGPT、桌面等同用户服务。[cgroup v2 文档](https://docs.kernel.org/admin-guide/cgroup-v2.html) 说明了进程后代的资源归属与内存/任务控制。

新增或修改测试时遵守以下约定：

- 集成测试启动进程使用 `tests/support/mod.rs` 的 `command` / `run`。默认 20 秒超时，stdout/stderr 各最多捕获 4 MiB；输入在限时范围内写入并关闭，无输入时接 `/dev/null`。错误、超时及正常退出都清理 Unix 进程组并回收主进程，Windows 尝试 `taskkill /T`；整组资源限制与最终清理由外层隔离环境保证。
- 所有直接 Bash 探针显式使用 `--noprofile --norc`，删除 `BASH_ENV`、`ENV`、`SSH_CLIENT`、`SSH_CONNECTION`、`SSH_TTY`。模拟缺失命令只修改子进程的 `PATH`，清理命令使用可靠的绝对路径；包装脚本仍须保留 `GEER_AGENT_SCRIPT` / `GEER_AGENT_ARG_*` 等内部参数，不能用 `env -i` 盲目删除。
- 测试不继承宿主网络标准输入、不读取宿主 Shell 启动脚本、不修改测试进程的全局环境。启动钩子回归使用临时目录中的无递归哨兵脚本；网络模拟仅监听本机临时端口，并设置连接/读写时限。
- 临时夹具使用 `std::env::temp_dir()`、`tempfile` 或本轮私有 `/tmp`、`/var/tmp`，不得把临时目录变量改到宿主路径来绕过隔离。异常退出由服务生命周期兜底；不要用 `rm -rf /tmp/geer-*` 等通配清理删除其他运行留下的文件。验证安全入口用 `make test-safety`，不要裸跑该回归脚本。
- `timeout`、`kill_on_drop(true)`、只杀直接子进程和单用户 `ulimit -u` 都不能单独保证系统安全。不要用 fork bomb、无限递归、持续无界输出等方式测试保护；用数量有限的后代、有限输出与临时文件验证退出和清理。进程组无法兜住主动脱离组的后代，因此仍需外层 cgroup 或等效隔离。
- 出现 Bash 数量持续增长、测试超时或内存异常时立即停止本次 `geer-agent-test-*.service`，检查其状态和遗留进程后再试。不要使用 `pkill bash` 等同用户全局清理，也不要靠禁用限额、增加 swap 或反复重跑来继续验收。
