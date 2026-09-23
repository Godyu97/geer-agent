# AGENTS.md

给编码代理的项目说明。人类文档以后放 `README.md`；OpenSpec 规划约束放 `openspec/config.yaml` 的 `context` / `rules` / `operations`。不要创建 `openspec/AGENTS.md` 或 `openspec/project.md`（OpenSpec 1.13+ 已废弃）。

## 项目是什么

`geer-agent` 是 [GeekAgent 教程](https://geektutu.com/books/geekagent) 的 **Rust 学习实现**。原教程用 TypeScript 从零做一个最小 Agent/Harness；本仓库用 Rust 跟同一条能力曲线，服务仓库所有者的 Rust 入门，而不是做生产级编码代理。

当前状态：Cargo 二进制 crate（edition 2024），`src/main.rs` 仍是 Hello World。功能按教程天数增量长出来。

## 先读哪份治理文件

| 文件 | 谁读 | 写什么 |
| --- | --- | --- |
| 本文件 | 日常编码代理 | 仓库约定、命令、安全、何时走 OpenSpec |
| `openspec/config.yaml` | OpenSpec 工作流（propose / apply / archive 等） | 规划用的项目背景、按产物规则、apply/archive 操作指引 |
| `openspec/specs/` | 实现与验收 | 已归档的行为契约（现在还是空的） |
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
cargo test
cargo fmt --all
cargo clippy --all-targets --all-features
```

改了代码再收工时：先 `fmt`，再相关 `test`，再 `clippy`。学习项目不要开 `-D warnings` 当门禁，但新引入的 clippy 警告要处理，不要留 `todo!()` / 无故 `unwrap`。

还没有 CI。本地命令就是质量门。

## Rust 约定

- Edition 2024；工具链以本机 `rustc`/`cargo` 为准，不要无故加 `rust-toolchain.toml`。
- 二进制 crate，入口 `src/main.rs`。模块按能力增长：`src/<module>.rs` 或 `src/<module>/mod.rs`，不要一上来铺 `domain/application/infrastructure`。
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
