# AGENTS.md

给编码代理的项目说明。人类文档以后放 `README.md`；OpenSpec 规划约束放 `openspec/config.yaml` 的 `context` / `rules` / `operations`。不要创建 `openspec/AGENTS.md` 或 `openspec/project.md`（OpenSpec 1.13+ 已废弃）。

## 项目是什么

`geer-agent` 是 [GeekAgent 教程](https://geektutu.com/books/geekagent) 的 **Rust 学习实现**。原教程用 TypeScript 从零做一个最小 Agent/Harness；本仓库用 Rust 跟同一条能力曲线，服务仓库所有者的 Rust 入门，而不是做生产级编码代理。

当前状态：Cargo 二进制 crate（edition 2024），已具备两种模型 API、工具循环、压缩、会话持久化、TUI 与可选 GUI。`src/main.rs` 调用 `ui::run`；文本界面位于 `src/ui/repl/`，三种界面通过 `interaction` 共用 Agent 的会话能力。功能继续按教程能力增量增长，架构现状见 `docs/design/architecture.md`。

## 先读哪份治理文件

| 文件 | 谁读 | 写什么 |
| --- | --- | --- |
| 本文件 | 日常编码代理 | 仓库约定、命令、安全、何时走 OpenSpec |
| `openspec/config.yaml` | OpenSpec 工作流（propose / apply / archive 等） | 规划用的项目背景、按产物规则、apply/archive 操作指引 |
| `openspec/specs/` | 实现与验收 | 已归档的行为契约 |
| `openspec/changes/` | 进行中的变更 | 提案、delta spec、设计、任务 |
| `.agents/skills/openspec-*/SKILL.md` | 用户点名 OpenSpec 时 | 工作流步骤；不要把 skill 正文抄进本文件 |

行为、架构、对外可见能力变化：先 OpenSpec，再写代码。用户明确说「先改代码 / 跳过 spec」时听用户的。纯格式、注释、拼写可以不建 change。

OpenSpec 技能已装在 `.agents/skills/`（vendor-neutral `agents` 目标）。用户说 `openspec propose` / `opsx propose` / `openspec apply` 等时，先读对应 skill，再用 CLI，不要手建 `openspec/changes/<name>/`。

常用入口：

- 还没想清楚：`openspec-explore`
- 已知要做什么：`openspec-propose`
- 按 `tasks.md` 落地：`openspec-apply-change`
- 改进行中的计划：`openspec-update-change`
- 只把 delta 合进主 spec：`openspec-sync-specs`
- 完成后归档：`openspec-archive-change`

默认 schema 是 `spec-driven`（proposal → specs → design → tasks）。规划产物用**中文**；Rust 标识符、模块名、commit 的 type 前缀用英文。

## 教程原则（必须守）

原教程三条规矩，Rust 版同样生效：

1. **最简单**：不要引入 Agent/CLI 框架，不要先做插件平台。核心依赖保持「调模型 + 读配置」起步，当天用不到的 crate 不要加。
2. **分天交付**：一次变更只做一个可运行的能力切片，当天结束必须能 `cargo run` 看到效果。
3. **只增量、不推翻**：在已有模块上长功能。禁止为了「更地道」而重写前一天已经能跑的路径，除非 change 标明 **BREAKING** 且用户同意。

把教程当行为与节奏来源，不要当 TypeScript 逐行翻译稿。模块划分用 Rust 习惯（`mod`、所有权、`Result`），不要复刻 `dayN/` 目录去堆一份 TS 的影子工程。

## 能力命名（OpenSpec specs）

主 spec 扁平放在 `openspec/specs/<capability>/spec.md`。能力 id 用 kebab-case，**不要**用 `day1` 这种日程号当 spec 名。日程号可以写在 proposal 里作对照。

| 教程 | 能力 id |
| --- | --- |
| Day1 REPL 地基 | `repl-chat` |
| Day2 工具调用循环 | `tool-loop` |
| Day3 Bash 工具 | `bash-tool` |
| Day4 文件工具与注册表 | `file-tools` |
| Day5 历史压缩 | `history-compaction` |
| Day6 多会话与持久化 | `session-persistence` |
| Day7 轻量 TUI 与用量 | `tui` |
| Day8 权限与回滚 | `permissions` |
| Day9 任务规划与子 Agent | `subagents` |
| Day10 项目指令与长期记忆 | `project-memory` |
| Day11 技能系统 | `skills` |
| Day12 代码搜索与网页抓取 | `search-fetch` |
| Day13 主动记忆 | `active-memory` |
| Day14 轻 RAG | `knowledge-rag` |
| Day15 MCP | `mcp` |
| Day16 插件框架 | `plugins` |

新能力先核对这张表和 `openspec list --specs`，能复用就复用，不要近义重复。表里没有的能力才新增 kebab-case id。

## 开发命令

```text
cargo build
cargo run
cargo fmt --all
make test
make clippy
```

根目录 `Makefile` 是这些命令的入口：`make help` 查看目标；收工用 `make check`（`fmt` → `test-safety` → `test` → `clippy`，顺序执行）。`test-safety` 需要 Python 3 标准库。测试与 clippy 使用独立受限服务，详见下节与 `README.md`。默认不加 `gui` / `embed-env`；GUI 用 `make gui`，内嵌 `.env` 用 `make embed`。

改了代码再收工时：先 `fmt`，再相关 `test`，再 `clippy`。学习项目不要开 `-D warnings` 当门禁，但新引入的 clippy 警告要处理，不要留 `todo!()` / 无故 `unwrap`。

还没有 CI。本地命令就是质量门。

## 测试的系统级安全（必须守）

2026 年 9 月 Fedora 曾发生全局 OOM 并终止 ChatGPT/Codex。已证实缺失命令测试的空 `PATH` 与 Bash 远程启动文件可触发 Fedora 缺失命令处理中的递归 fork；历史证据不能证明每次 OOM 都来自同一测试。把子进程、线程、输出和编译峰值都纳入测试安全范围。

- **先隔离再运行**：开发主机上测试必须用 `make test` / `make gui-test` / `make check` 或 `scripts/test-safe.sh`。禁止裸跑 `cargo test`、`npm test`、测试二进制或未受限的危险复现。脚本默认独立 cgroup：`MemoryMax=4G`、`MemorySwapMax=0`、`TasksMax=256`、`RuntimeMaxSec=10min`、`TimeoutStopSec=5s`、`KillMode=control-group`、`OOMPolicy=kill`；编译默认 2 并行、Rust 测试默认 1 线程，Vitest 固定 1 worker 且关闭文件并行。必须确认实际限制有效；入口失败不得绕过。没有 systemd/cgroup 的环境先提供等效隔离。
- **先单项再完整**：Shell、PATH、超时与清理代码修改后，先运行 `make test-safety`，再受限运行缺失命令单项和 `process_safety` 回归，最后跑完整检查。`test-safety` 的探针必须各自受限且顺序执行；组内 OOM 仅允许固定上界的分配。不得执行无约束递归来证明修复，也不要并发运行多组完整测试叠加额度。
- **隔离启动环境**：直接 Bash 测试探针必须带 `--noprofile --norc`，清除 `BASH_ENV ENV SSH_CLIENT SSH_CONNECTION SSH_TTY`。清空 `PATH` 只对受控子进程生效；保留 `GEER_AGENT_SCRIPT` / `GEER_AGENT_ARG_*` 等工具内部参数，禁止用 `env -i` 误删。哨兵启动文件只建在测试临时目录，不读取或改写宿主 `~/.bashrc`、`/etc/profile.d/`。
- **标准输入明确**：无输入时 `stdin(Stdio::null())`；有 REPL 输入时使用管道、限时写入并显式 EOF。不得把远程网络连接或交互终端继承给测试探针。
- **整组清理且回收**：集成测试通过 `tests/support/mod.rs` 的 `command` / `run` 启动进程，默认 20 秒超时、stdout/stderr 各 4 MiB 上限。超时、错误、panic 与正常结束都清理后代并回收主进程；Unix 建独立进程组，清理工具用绝对路径。`kill_on_drop(true)` 或只杀主进程不能代替进程组/cgroup 清理；主动脱离组的后代由外层隔离兜底。
- **临时文件随服务回收**：安全入口必须设置 `PrivateTmp=disconnected`，为每轮服务提供独立 tmpfs 的 `/tmp` 与 `/var/tmp`，固定 `TMPDIR TMP TEMP=/tmp`，核对实际文件系统与宿主设备号，隔离失败不得继续。临时夹具使用标准库临时目录或本轮私有路径，不得改环境绕过隔离。正常、失败、panic、超时、SIGKILL 和组内 OOM 后由 systemd 回收文件与 tmpfs，不能只依赖析构或 `trap`；启动脚本被 SIGKILL 时由有限运行时限加 5 秒停止等待兜底。文件内存仍计入测试组额度，正常路径应及时删除以降低峰值。禁止通配删除宿主 `/tmp` 下的项目文件或清理不属于本轮的路径。
- **复现必须有限**：仅用少量后代、有限输出和临时文件做清理验证，禁止 fork bomb、无限递归、无界输出。测试网络仅用本机临时端口，连接/读写必须有时限；不得修改全局环境影响并行测试。
- **额度只覆盖测试组**：根据编译峰值调整 `GEER_TEST_MEMORY_MAX` / `GEER_TEST_TASKS_MAX` / `GEER_TEST_RUNTIME_MAX` 的有限值，为桌面/ChatGPT 留出余量。不得设置 `infinity`、取消安全限制、修改整个 user slice 的额度或靠增加 swap 掩盖异常。不要改宿主系统服务来完成测试修复。
- **异常先停止**：看到持续增殖的 Bash、超时或内存异常，停止本次命名服务并确认资源已回收，查明原因再重试。禁止 `pkill bash` 等全局清理或盲目反复运行测试。

## Rust 约定

- Edition 2024；工具链以本机 `rustc`/`cargo` 为准，不要无故加 `rust-toolchain.toml`。
- 二进制 crate，入口 `src/main.rs`。模块按能力增长：`src/<module>.rs` 或 `src/<module>/mod.rs`，不要一上来铺 `domain/application/infrastructure`。
- 业务模块不依赖具体 UI：`config` 可被所有模块引用；`provider` 不引用 `tools` / `agent` / `ui`；`tools` 不引用 `provider` / `agent` / `ui`；`agent` 实现 `interaction::Session`，组合模型、工具与会话，不引用 `ui`。`interaction` 定义中立契约并执行共用命令，可使用会话数据类型，但不导入具体 Agent 或 UI。`ui::run` 与 GUI 工作线程是组合点，可创建 Agent、注入授权；`ui/repl` / `ui/tui` / GUI 命令适配通过 `interaction::execute` 执行消息和会话写操作，读取绘图数据可直接使用 Session。入口 `main` 只调用 `ui::run`。
- 标识符英文；注释只写「为什么」和 Rust 初学者不容易看出来的所有权/生命周期/错误处理选择，用中文。
- 库路径与可失败逻辑用 `Result`/`Option`。`unwrap`/`expect` 仅限「这是 bug」或测试。禁止 `unsafe`。
- 优先标准库。新 crate 必须能回答「std 为什么不够」。异步、流式输出、HTTP 客户端等在对应 change 的 design 里论证后再加。
- 公开行为用测试钉住：单元测试跟模块走，REPL/流式/工具调用等用集成测试或可重复的手工验收步骤（写进 spec scenario / task 验证）。
- 配置对齐教程语义：OpenAI 兼容的 `base_url` / `api_key` / `model`，从环境变量读取；缺 key 时明确退出，不要静默假成功。

## Git 工作流

不要 invent 分支策略。只有用户明确说时才跑下面的 skill：

- `coding begin` / `coding end`（含 `codeing` 拼写）→ `.agents/skills/git-team-workflow/SKILL.md`
- `upd dev` → `.agents/skills/git-upd-dev/SKILL.md`
- 进行中的 merge/rebase 冲突 → `.agents/skills/resolving-merge-conflicts/SKILL.md`

Git skill 与业务/OpenSpec 上下文隔离：那些 skill 只要 Git 元数据，不靠本文件做决策。未点名时：小步提交、不要 force-push、不要改 Git config、不要提交 `target/`、`.pi/`、密钥。

## 安全

- 永远不要提交 API key、`.env`、会话记录里的用户数据。
- 模型只走用户配置的 OpenAI 兼容接口；不要把密钥写进源码或 OpenSpec 产物。
- 工具（bash、写文件、MCP）默认视为危险能力：后续实现必须可关闭、可确认，不能默认对仓库外路径动手。

## 明确不要做的事

- 不要为了「完整 Agent 产品」提前做云端、账号系统、多租户、电子市场。
- 不要添加与当天能力无关的抽象层、feature flag 框架、自定义 proc macro。
- 不要在 OpenSpec 产物里贴 `openspec/config.yaml` 的 `context`/`rules` 原文。
- 不要手改 OpenSpec 生成的 `.agents/skills/openspec-*` skill；刷新用 `openspec update`。
- 不要把教程 TypeScript 依赖（`openai` npm 包、`tsx`、`readline`）按名字搬进 Cargo，只搬语义。
