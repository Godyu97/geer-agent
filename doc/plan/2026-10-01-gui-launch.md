# GUI 无额外终端启动：问题原因与执行方案

## 问题原因

此前 `gui` feature 只负责纳入 Tauri 及其依赖，同一个程序仍按终端应用构建。旧方案明确保留 Windows 控制台子系统；从资源管理器双击这样的 exe，Windows 会创建控制台，随后才显示 GUI。终端创建发生在进入 Rust `main` 之前，因此运行后隐藏窗口会有闪烁，按 `.env` 动态决定链接子系统也不可行。[Rust 的子系统说明](https://doc.rust-lang.org/reference/runtime.html#the-windows_subsystem-attribute)解释了 `windows` 子系统用于避免 GUI 启动时显示控制台。

另外，启动会执行 Git Bash 版本与 Windows 系统版本探测；后续工具执行及超时清理还会启动 Bash、cmd 或 taskkill。仅隐藏父进程窗口，不能阻止这些控制台子程序新建窗口。标准流接管道也不等于无控制台。[Windows 进程创建标志](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags)提供 `CREATE_NO_WINDOW` 来启动无控制台的后台程序。

`make gui` 和 `cargo run` 是开发命令，会占用用户已经打开的终端。这种终端属于调用者，不应由应用关闭。Linux 本身不会因普通二进制启动而自动创建终端；若桌面启动器要求终端或调用终端模拟器，需要改为直接执行二进制并设置 `Terminal=false`。以上基于代码和既有设计确认启动机制；实际窗口表现另列真机验收，不与编译检查混淆。

## 执行的技术方案

1. 增加 `desktop-gui` Cargo feature，自动包含 `gui`。仍使用同一个 crate 和 `main → ui::run` 入口，未新增依赖或第二个 Rust 二进制。普通 `gui` 构建保留开发时的 GUI/TUI/REPL 选择。
2. Windows 桌面构建设置 `windows_subsystem="windows"`，debug/release 均生效，从系统加载进程起不创建控制台。桌面构建的 `auto` 与 `gui` 都进入 GUI；误配置为 `tui` / `repl` 会在 GUI 提示改用终端构建。
3. `config::background_command` 集中创建生产后台命令，Windows 用 Tokio 的安全接口设置 `CREATE_NO_WINDOW=0x08000000`。覆盖 Bash/系统版本探测、Bash 工具和超时清理。继续使用原有输出管道、输出上限、超时、进程组和回收逻辑；不引入 `unsafe`，不采用会改变进程关系的 `DETACHED_PROCESS`。用户主动执行终端模拟器不在自动控制台抑制范围内。
4. 桌面入口捕获环境加载及界面模式解析错误，将拥有的错误文本传入 GUI 工作线程，复用 `StartupError` 与等待前端连接的路径。缺少 API key 等 Agent 初始化错误继续由已有工作线程在窗口中显示；错误状态仍能正常关闭窗口。模型、流式输出、授权和数据库行为不变。
5. 增加 `make gui-build`：先构建前端，再构建 release 桌面产物。提供 `src/ui/gui/geer-agent.desktop` 模板直接执行 Linux 产物，`Terminal=false`。不自动写入用户桌面配置。

后台命令仍调用配置的 Bash 路径，未绕过 Git 启动器；[Git for Windows 启动器源码](https://github.com/git-for-windows/MINGW-packages/blob/main/mingw-w64-git/git-wrapper.c)在没有控制台时也以 `CREATE_NO_WINDOW` 启动实际程序。Windows 真机仍需验证所安装 Git 版本的表现。

## 使用方式

```text
make gui-build
```

Windows：直接双击 `target/release/geer-agent.exe`；无需 cmd、PowerShell 或 bat 包装。Linux：把桌面模板的 `Exec` 替换为 release 产物绝对路径，保留双引号，将模板保存到 `~/.local/share/applications/geer-agent.desktop` 后从应用菜单打开。

`.env` 保持既有查找规则：二进制同级优先，本地 Cargo 构建可回退项目目录，之后是 `~/.geer-agent/`；不依赖启动器工作目录。`GEER_AGENT_UI` 可留空、设为 `auto` 或 `gui`。终端会话继续使用不含 `desktop-gui` 的构建；不同构建会覆盖同一路径下同名产物，分发时需选择正确版本。

## 验证与人工验收

自动验证通过受限服务执行，实际检查 cgroup 内存、swap、任务数与私有 tmpfs，不裸跑测试。先安全入口回归，再缺失命令、进程清理和 Bash 回归，最后完整质量门、GUI 模式检查及 Windows 交叉检查。

已执行结果：

- `make test-safety`：正常、失败、SIGKILL、超时、组内 OOM、启动脚本中断及隔离预检全部通过。
- 缺失命令单项、`process_safety` 的 4 项、Bash 的 4 项回归通过；命令输出、非零退出、超时、后台后代回收及字面参数保持原行为。
- `make check`：格式、安全入口、普通构建的 233 项测试及 clippy 通过，1 项真实网络验收继续忽略。
- 受限前端生产构建及 `make gui-test`：类型检查和构建通过，24 项前端测试通过；新增启动失败在会话连接前可见、输入区域不可发送的回归。
- `make test FEATURES=desktop-gui`：190 项测试通过，1 项真实网络验收忽略；`make clippy FEATURES=desktop-gui` 通过。依赖终端输入输出的集成测试仅在普通构建中运行。
- 受限 Windows GNU 交叉构建 `cargo build --target x86_64-pc-windows-gnu --features desktop-gui` 通过；`objdump -p` 检查实际 debug exe 的 `Subsystem=00000002 (Windows GUI)`，确认桌面特性在 debug 也已生效。产物位于 `target/x86_64-pc-windows-gnu/debug/geer-agent.exe`。这是交叉构建，未执行 Windows 程序。
- Windows 目标的 `cargo clippy --all-targets --target x86_64-pc-windows-gnu --features desktop-gui` 通过；最后再次执行 `make check` 通过，无新增 clippy 警告。
- `make -n gui-build` 确认先前端后 release 桌面构建；`desktop-file-validate` 验证 Linux 模板通过，OpenSpec 严格校验通过。Linux release 全量构建与 Windows 真机视觉验收未执行。

Windows 真机步骤：

1. 构建桌面版本，`.env` 中设有效模型配置与 `GEER_AGENT_UI=auto`，双击 exe：仅出现 GUI，启动期间也无控制台闪烁；debug 桌面构建同样验收。
2. 在 GUI 中授权执行 `printf hello`：输出出现在 GUI；执行 `sleep 30`：超时清理正常且没有新控制台。检查 cmd/Bash 启动探测同样没有闪烁。
3. 在测试目录放置无效 `.env`，再分别测试无效 UI 值、误选 `tui` / `repl`、缺 key：窗口显示具体失败原因且能关闭。
4. 使用普通终端构建：交互 TUI 和管道 REPL 正常。

Linux 真机步骤：替换并安装桌面模板，从应用菜单启动；检查无新终端，配置错误在 GUI 显示，关闭正常。用已有终端运行开发命令时，原终端继续保留。

跨目标编译检查不能替代 Windows 实际双击与窗口闪烁验收；当前主机为 Linux，Windows 视觉结果需明确标为未实测。
