# 项目、技术栈与运行环境

GeekAgent 教程的 Rust 学习实现，提供模型工具循环、会话与记忆管理，以及 REPL/TUI 和可选 GUI/Web。模块入口见 [索引](../docs/mod/00-index.md)。

## 应用与工程组件

| 范围 | 语言与角色 | 原始证据与边界 |
| --- | --- | --- |
| [Rust 应用](../src) | Rust 2024，单一二进制 crate，非 workspace | [Cargo.toml](../Cargo.toml)、[main.rs](../src/main.rs)。默认终端配置；Ratatui/Crossterm 实现 TUI，`gui` 用 Tauri 2，`web` 用 Axum，`desktop-gui` 包含 `gui`。 |
| [共用前端](../src/ui/frontend) | TypeScript/React，单一私有包 | [package.json](../src/ui/frontend/package.json)、[Vite 配置](../src/ui/frontend/vite.config.ts)。Bun 1.4.2、React、Vite、tsc、Vitest；没有第二套 Web App。 |
| [开发工具与夹具](../docs/mod/dev-tooling/00-overview.md) | POSIX Shell、Python 3、Rust 测试支持 | [Makefile](../Makefile)、[scripts](../scripts)、[tests/support](../tests/support)。安全脚本使用 systemd/cgroup v2；治理检查器使用 Python 3.9+ 标准库。 |
| [仓库内工作流](../.agents) | 已安装的 Markdown skill、Python Git runner | [.openspec-target](../.agents/skills/.openspec-target)、[OpenSpec 配置](../openspec/config.yaml)。OpenSpec skill 由 CLI 刷新，Git runner 仅在对应口令授权后运行。 |

HTTP 模型调用用 async-openai/reqwest；SQL 用 SeaORM，MongoDB 用官方 Rust 驱动。依赖与平台条件以清单、调用链及 README 核对，不从名称推断已运行结果。

## 命令与运行条件

下表均从仓库根目录执行，定义以 [Makefile](../Makefile)、[Cargo.toml](../Cargo.toml)、[前端 scripts](../src/ui/frontend/package.json) 为准。此轮仅核对项目命令定义，未运行 Rust/前端业务构建、测试或 clippy；治理检查见 [维护手册](04-documentation.md)。

| 用途 | 命令 | 前提、产物与副作用 |
| --- | --- | --- |
| 查看实际入口 | `make help` | 只展示 Make 目标，不启动应用。 |
| 默认终端构建/运行 | `cargo build` / `cargo run` | 无需静态前端；运行需模型配置与 Bash，可能初始化所启用的数据库；模型请求只走用户配置端点。 |
| 全界面构建/release/运行 | `make build` / `make release` / `make run` | 自动准备两种前端并启用 `gui,web`，需要 Bun 和平台 Tauri 依赖。前两项只构建，`run` 启动。 |
| 前端依赖/构建 | `make frontend-deps` / `make frontend-build` | 冻结安装 `bun.lock`，安装脚本并行 1；构建先 tsc，再分别输出 `dist/gui`、`dist/web`。可能获取依赖，会写依赖目录和产物。 |
| 格式/只检查格式 | `make fmt` / `make fmt-check` | 前者改写 Rust 格式；后者只检查。 |
| 定向 Rust 测试 | `make test TEST=名称` 或 `make test TEST_ARGS='--test tool_loop'` | 受限服务内编译和运行；`FEATURES` 默认空，可显式选择所需 feature。 |
| 默认 Rust clippy | `make clippy` | 受限 `--all-targets`，默认无 UI feature；处理新增警告，不加 `-D warnings` 门禁。 |
| 共用前端检查/测试 | `make frontend-check` / `make frontend-test` | 前者准备依赖并检查类型；后者连依赖准备也受限，Vitest 单 worker、关闭文件并行。 |
| Web 验证 | `make web-test` | 顺序执行受限前端测试、前端构建，再运行 `FEATURES=web` 的 Rust 测试；`tests/web_ui.rs` 还要求 Unix 且未启用 `desktop-gui`。 |
| 全界面 clippy | `make clippy-all` | 先受限准备前端，再受限检查 `gui,web`；不使用 `--all-features`。 |
| 安全入口验证 | `make test-safety` | 受限 Python 控制服务顺序运行有限探针，覆盖预检拒绝及异常回收；不能裸跑脚本。 |
| 代码变更收工 | `make check` | 顺序执行 fmt → test-safety → test → clippy；默认终端配置。本项目无 CI，本地质量门不替代 UI/平台验收。 |

所有测试与 clippy 执行前必须读 [README 测试与系统安全](../README.md#测试与系统安全)。入口核实实际内存/任务限制、禁用 swap 和独立 tmpfs；不成立则拒绝启动。Linux/systemd/cgroup v2 以外环境先提供等效隔离，不能回退裸跑。Shell/PATH/进程/超时/清理修改按 README 的安全回归顺序执行，不并发叠加测试服务。

默认限制是本轮 4 GiB、256 任务、10 分钟，编译 2 并行、Rust 测试 1 线程；只用 `GEER_TEST_MEMORY_MAX` / `GEER_TEST_TASKS_MAX` / `GEER_TEST_RUNTIME_MAX` 调整有限本轮额度。隔离实现和夹具边界见 [开发工具模块](../docs/mod/dev-tooling/01-implementation.md)。

Windows 的 `make build/release` 另构建 `desktop-gui`，与通用程序并存交付 `geer-agent-desktop.exe`；桌面缓存在 `target/desktop-gui`，根目录可由 `CARGO_TARGET_DIR` 指定。该产物只支持 GUI；普通构建的 `auto` 才按终端选择 TUI/REPL。可选 UI 的直接 Cargo 构建先 `make frontend-build`；`embed-env` 单独直接用 Cargo，不添加旧 GUI/Web Make 别名或 `make debug`。

## 环境与配置

配置事实来自 [src/config](../src/config) 和 [README 配置](../README.md#配置与运行)。进程环境优先；dotenv 候选依次为可执行文件旁、仓库开发构建的项目根、用户 `~/.geer-agent`，只选择一份磁盘文件；显式 `embed-env` 再补编译内嵌默认值。默认 SQLite 位于所选程序目录的 `.db/geer.sqlite`，不假定总在仓库根。

`OPENAI_API_KEY`、`OPENAI_MODEL` 必填；`OPENAI_BASE_URL` 默认 `https://api.openai.com/v1`，`OPENAI_API` 选择 responses/chat-completions。`GEER_AGENT_UI` 选择 auto/gui/web/tui/repl；数据库、会话、记忆、工具授权与预算通过 `GEER_AGENT_*` 配置。Web 默认监听 `0.0.0.0:8827`，端口/口令键是 `GEER_AGENT_WEB_PORT` / `GEER_AGENT_WEB_TOKEN`。只记录键名和加载链，不读取或保存凭据值。

DAO 外部数据库契约仅在提供专用 `GEER_AGENT_TEST_POSTGRES_URL` / `GEER_AGENT_TEST_MYSQL_URL` / `GEER_AGENT_TEST_MONGODB_URL` 时执行；未配置时函数可直接返回，测试整体通过不能证明三种后端已联机验收。默认忽略的真实网络 smoke 同理，下一步见 [存储事项](../docs/mod/storage/02-issues.md)。
