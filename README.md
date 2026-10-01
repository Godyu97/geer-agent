# geer-agent

GeekAgent 教程的 Rust 学习实现。

## 配置与运行

普通 `cargo run` 可使用项目根目录的 `.env`：复制 `.env.example` 为 `.env`，填写 `OPENAI_API_KEY` 和 `OPENAI_MODEL`，然后运行：

```sh
cargo run
```

复制可执行文件到其他目录运行时，优先读取可执行文件同级的 `.env`；若不存在，则读取 `~/.geer-agent/.env`。普通本地 Cargo 构建产物在同级没有 `.env` 时，也会检查项目根目录的 `.env`。启动工作目录中的其他 `.env` 不参与查找。所有候选位置都没有外部 `.env` 时，也可以只设置同名进程环境变量。`OPENAI_BASE_URL` 可选，默认使用 OpenAI 地址；`OPENAI_API` 可选，默认使用 Responses API，另可设为 `chat-completions`。

`GEER_AGENT_UI=auto|gui|tui|repl` 可写在上述位置的 `.env`，进程环境变量优先。默认 `auto` 在交互终端使用 TUI，在输入或输出接管道时使用文本 REPL；显式 `tui` 要求交互终端。`gui` 必须在构建时启用 Cargo 的 `gui` feature，否则程序会提示构建方式。三种界面继续使用同一个 `geer-agent` 可执行文件。

## 桌面 GUI（可选）

GUI 使用 Tauri 2 + React/TypeScript。先构建静态前端，再启用 Rust feature：

```sh
cd src/ui/gui/frontend
npm ci
npm run build
cd ../../../..
cargo run --features gui
```

在程序实际选用的 `.env` 中设置 `GEER_AGENT_UI=gui` 后，上述 `cargo run` 会打开窗口。未设置时仍按 `auto` 选择终端界面；也可以执行 `GEER_AGENT_UI=gui cargo run --features gui` 临时启动。GUI 提供完整会话记录、侧栏会话切换、Markdown 回答、流式输出、用量与工具授权。现有 `/help`、`/new`、`/open`、`/save`、`/compact`、`/sessions`、`/exit` 等命令可直接在输入框使用。Enter 发送，Shift+Enter 换行；授权默认拒绝。关闭窗口时等待正在执行的请求并补写会话，保存失败时可重试、返回或明确退出。若持久化已关闭或数据库不可用，关闭含有对话的仅内存会话前会提示数据无法保存。

日常桌面使用请先执行 `make gui-build`，再直接打开 `target/release/geer-agent.exe`（Windows）或 `target/release/geer-agent`（Linux）。该构建启用 `desktop-gui`，默认 `auto` 直接进入 GUI；Windows 从启动起不创建控制台，后台版本探测、工具执行与超时清理也不弹出控制台。`GEER_AGENT_UI=gui` 同样有效，配置错误在窗口内显示。桌面构建只支持 GUI；需要 TUI/REPL 时使用普通 `make run` 或不含 `desktop-gui` 的构建。`make gui` / `cargo run --features gui` 仍是从已有终端运行的开发入口，调用者的终端会继续保留。

Linux 桌面入口模板位于 `src/ui/gui/geer-agent.desktop`：把 `Exec` 改为桌面可执行文件的实际绝对路径，保留双引号及 `Terminal=false`，然后保存到 `~/.local/share/applications/geer-agent.desktop`，从应用菜单打开。启动器直接运行二进制，不调用 Cargo 或终端模拟器。配置仍按既有 `.env` 查找顺序加载，无需以项目目录作为启动工作目录。原因、技术方案及验收步骤见 [GUI 启动优化方案](doc/plan/2026-10-01-gui-launch.md)。

Linux 需要 WebKitGTK 4.1 等 Tauri 开发依赖。Fedora 按 [Tauri 官方前置要求](https://tauri.app/start/prerequisites/) 安装 `webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel` 及 C 开发工具；如编译提示找不到 `dbus-1.pc`，还需 `dbus-devel`。从 Wayland 会话中运行，可用 `GDK_BACKEND=wayland` 做原生 Wayland 验收。Windows 11 使用 MSVC Rust 工具链、Microsoft C++ Build Tools、WebView2 和 Node.js；本项目工具执行还需要 Git for Windows Bash。前端产物嵌入启用 GUI 的二进制，修改前端后需重新运行 `npm run build` 和 Cargo 构建。默认 `cargo build` 不需要 Node 或 GTK/WebKitGTK。

侧栏的“管理会话”支持勾选、全选当前列表和批量删除；切换“当前 / 全部”范围会清空选择。确认弹窗显示标题、完整 UUID 和当前会话替换提示，默认取消。删除其他会话或取消操作会保留聊天草稿；删除当前会话成功后，消息区和草稿清空。失败项可点击“重试删除 / 清理”。

## 终端界面

在交互终端运行 `cargo run` 会进入全屏 TUI：左侧是可滚动的消息区，底部输入，宽度至少 60 列时右侧显示模型、会话、上下文估算与 token 用量。终端变窄时会隐藏面板。方向键编辑输入，PageUp/PageDown 或上下方向键滚动消息；`/help`、`/new`、`/sessions`、`/open`、`/compact` 等命令沿用原语义。工具首次授权会在界面内要求 `[y/N]`，只有输入 `y` 或 `yes` 才会同意。

面板的上下文 token 与占比是估算值；流式回答期间用 `~` 标记临时用量，完成后显示接口报告的真实 token 数。本轮统计包含该条输入触发的模型和自动压缩请求，累计统计涵盖本进程中的会话及手动压缩；若兼容接口未提供某次用量，面板显示“用量部分缺失”，估算值不会计作真实用量。`/exit`、Ctrl+C、Ctrl+D 会尝试保存会话并恢复原终端；全屏期间暂存的诊断记录在退出后写到 stderr。输入或输出接管道时继续使用原文本 REPL。

F3 或 `/sessions [--all]` 打开会话管理面板并保留聊天草稿。上下方向键、PageUp/PageDown 移动，Space 勾选，`a` 全选/取消全选，Delete 请求删除，Enter 打开高亮会话，Tab 切换范围，Esc 返回聊天。删除确认时 Enter/Esc 取消，`y` 删除；结果页可滚动，`r` 重试失败项，Enter/Esc 返回列表。

## 工具调用

三种界面向模型提供 11 个工具：`get_current_time`、`bash`、`ls`、`glob`、`rg`、`read`、`write`、`edit`、`search`、`web_search`、`web_fetch`。模型选择工具后，程序执行并把结果交回模型继续回答。`GEER_AGENT_TOOLS=off` 可在启动时关闭所有工具；默认开启。服务端模型需要支持所选 API 的函数工具调用协议。

`read` 接受 `path`、可选的 1 起始行号 `offset` 和行数 `limit`，单次最多返回 2000 行、50 KiB 的 UTF-8 文本。`write` 接受 `path` 与 `content`，创建或覆盖文件。`edit` 接受 `path` 和 `edits` 数组，其中每项是 `oldText`、`newText`；旧文本必须在原文件中唯一匹配，各项不能重叠。`bash` 接受 `command`，在当前 workspace 运行，10 秒超时，结果最多 2000 字符。

`GEER_AGENT_BASH_BIN` 可指定 Bash 可执行文件的绝对路径。未设置或为空时，Windows 优先使用 `C:\Program Files\Git\bin\bash.exe`，该文件不存在时再退回 `PATH` 中的 `bash`；Linux 等其它系统直接使用 `PATH` 中的 `bash`。这样可以避开 Windows System32 里的 WSL `bash.exe`。工具会自动把 Git 自带的 `usr\bin` 加入命令的 `PATH`，并把 workspace 以 `F:/repo` 形式传给 Bash。启动时会验证所选 Bash 并读取版本，路径或版本无效时直接报错退出。每次模型请求都会带默认系统提示，其中 `<context_data>` 包含系统版本和所选 Bash 版本；`/reset` 后仍会提供这些环境信息。

Bash、目录查询、文件工具和 `search` 各自在当前会话首次使用时请求 `y/N` 授权。每次切换会话（包括切回旧会话）都会清空授权；打开失败或打开当前会话则保持授权。非交互输入无法确认时默认拒绝执行。

`ls` 列目录，`glob` 找路径，`rg` 搜正文，固定查询都通过配置的 Bash 执行，需要其 `PATH` 中有 `ls` / `rg`。这些已有工具保留任意路径授权范围。新增 `search` 只搜索当前 workspace：默认 `path="."`、`output="content"`、区分大小写的正则，可选 `glob`、`fixed_strings`、`ignore_case`；`output="files"` 只返回命中文件。结果为 `相对 workspace 路径:行号: 原文`，保留缩进，子目录结果也能直接交给 `read`。规范化后越界的路径与链接会被拒绝，不跟随目录链接；沿用 rg 忽略规则，显式路径或 glob 可覆盖默认过滤，跳过二进制及超过 1 MiB 的文件。最多 50 项、总输出 2000 字符，截断时只保留完整命中行并提示缩小范围。

`web_search` 接受 `query` 和可选 `num_results`（默认 5，范围 1–20），通过 [Exa 免 Key MCP](https://exa.ai/docs/get-started/exa-mcp) 返回标题、完整 URL、摘要和后端名称。首次会话授权会明确说明查询将发送给 Exa；服务的 HTTP、RPC 或超时错误会如实报告。`web_fetch` 接受完整 HTTP(S) `url`，直接抓取 HTML、Markdown、JSON 等文本，返回来源与最终 URL、响应信息及可读正文；HTML 清除脚本、样式和标签，保留段落、列表与代码缩进。支持公网、本机和内网地址，每次访问都显示完整 URL 并确认，跨来源重定向再次确认，最多跟随 5 次。拒绝后停止访问目标。

网页搜索网络超时 25 秒，抓取网络总耗时 15 秒（用户确认等待不计入），响应体上限 1 MiB，单次结果上限 12000 字符。客户端复用 HTTP/TLS 与环境代理配置，不携带模型 API key、Cookie 或用户认证头。网页摘要和正文作为不可信数据交给模型。首版只提供静态文本访问；PDF、JavaScript 渲染、缓存及抓取回退暂未实现。

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

会话和 Trace 默认共用所选配置目录的 `.db/geer.sqlite`，目录会自动创建：普通 `cargo run` 使用项目根目录时写入项目的 `.db/`；可执行文件旁有 `.env` 时写入其同级 `.db/`；使用 `~/.geer-agent/.env`，或仅使用进程变量、内嵌配置时写入 `~/.geer-agent/.db/`。项目根目录的 `.db/` 已加入 Git 忽略。可用公共配置改用其他数据库或指定旧文件：

```sh
GEER_AGENT_DATABASE_URL='sqlite:///absolute/path/to/previous/.db/geer.sqlite?mode=rwc'
```

`GEER_AGENT_DATABASE` 默认为 `sqlite`；选择其他后端时必须设置对应 URL。省略 `GEER_AGENT_DATABASE_URL` 才会使用上述同目录默认路径。Trace 和会话共用这组数据库配置，可分别通过 `GEER_AGENT_TRACE` 和 `GEER_AGENT_SESSION_PERSISTENCE` 关闭。已有数据库可以通过公共配置指定原地址，无需迁移数据。

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

```sh
make help      # 列出目标
make check     # fmt -> test-safety -> test -> clippy
make run       # 终端 TUI / REPL
make gui       # 构建前端并以 GUI feature 运行
make gui-build # 构建日常桌面使用的 release 产物
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
./scripts/test-safe.sh make gui-frontend
make test FEATURES=gui
make test FEATURES=desktop-gui
make gui-test
```

桌面构建的测试覆盖共用业务与模式选择；依赖 REPL 输入输出的进程集成测试仅在普通构建中运行，由 `make check` 验证。

`make test`、`make gui-test`、`make clippy` 和 `make clippy-all` 通过 `scripts/test-safe.sh` 启动独立的 systemd 用户服务。Cargo 编译、测试程序与后代进程都归入本次服务的 cgroup；前端测试安装依赖和 GUI clippy 准备前端的阶段也先进入受限服务。入口检查内核实际的内存、swap、任务限制和临时目录挂载，缺少 systemd 用户服务、cgroup v2、有效限制或临时目录隔离时直接失败，不自动执行无约束测试。`make check` 按 fmt → test-safety → test → clippy 顺序执行，即使传入 `make -j` 也保持这个顺序。

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

需要其他 Cargo 参数时，用 `TEST_ARGS`；需要直接验证其它命令时，用 `./scripts/test-safe.sh <命令> [参数...]`。不要直接在开发主机运行裸 `cargo test`、`npm test` 或测试二进制，也不要同时启动多组测试去叠加资源额度。Windows、macOS 和没有 systemd 用户服务的环境需要先提供带内存、进程数量、总时限和后代清理能力的独立测试环境；受限入口不会静默降级。不要对整个 `user.slice` 设置测试额度，否则会一起限制 ChatGPT、桌面等同用户服务。[cgroup v2 文档](https://docs.kernel.org/admin-guide/cgroup-v2.html) 说明了进程后代的资源归属与内存/任务控制。

新增或修改测试时遵守以下约定：

- 集成测试启动进程使用 `tests/support/mod.rs` 的 `command` / `run`。默认 20 秒超时，stdout/stderr 各最多捕获 4 MiB；输入在限时范围内写入并关闭，无输入时接 `/dev/null`。错误、超时及正常退出都清理 Unix 进程组并回收主进程，Windows 尝试 `taskkill /T`；整组资源限制与最终清理由外层隔离环境保证。
- 所有直接 Bash 探针显式使用 `--noprofile --norc`，删除 `BASH_ENV`、`ENV`、`SSH_CLIENT`、`SSH_CONNECTION`、`SSH_TTY`。模拟缺失命令只修改子进程的 `PATH`，清理命令使用可靠的绝对路径；包装脚本仍须保留 `GEER_AGENT_SCRIPT` / `GEER_AGENT_ARG_*` 等内部参数，不能用 `env -i` 盲目删除。
- 测试不继承宿主网络标准输入、不读取宿主 Shell 启动脚本、不修改测试进程的全局环境。启动钩子回归使用临时目录中的无递归哨兵脚本；网络模拟仅监听本机临时端口，并设置连接/读写时限。
- 临时夹具使用 `std::env::temp_dir()`、`tempfile` 或本轮私有 `/tmp`、`/var/tmp`，不得把临时目录变量改到宿主路径来绕过隔离。异常退出由服务生命周期兜底；不要用 `rm -rf /tmp/geer-*` 等通配清理删除其他运行留下的文件。验证安全入口用 `make test-safety`，不要裸跑该回归脚本。
- `timeout`、`kill_on_drop(true)`、只杀直接子进程和单用户 `ulimit -u` 都不能单独保证系统安全。不要用 fork bomb、无限递归、持续无界输出等方式测试保护；用数量有限的后代、有限输出与临时文件验证退出和清理。进程组无法兜住主动脱离组的后代，因此仍需外层 cgroup 或等效隔离。
- 出现 Bash 数量持续增长、测试超时或内存异常时立即停止本次 `geer-agent-test-*.service`，检查其状态和遗留进程后再试。不要使用 `pkill bash` 等同用户全局清理，也不要靠禁用限额、增加 swap 或反复重跑来继续验收。
