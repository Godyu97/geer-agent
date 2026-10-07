# AI 编码工作流

任务意图选择流程，风险选择验证强度。授权范围内持续推进，执行前确认如何观察成功；定位方式见 [上下文路由](00-context.md)。

## 任务路由

| 意图 | 执行与记录 |
| --- | --- |
| 只读解释、诊断或审查 | 定向核对事实、交付证据；不写工作区、不生成索引。 |
| 格式、注释、拼写、治理文档补齐 | 直接修改，检查结构/链接/规则及差异；不建 change 或永久流水。 |
| 契约内 Bug、保持行为的重构 | 确认原契约与失败条件，最小修复、定向验证；若涉及架构或有意改变行为，转下行。稳定事实改变才同步模块正文。 |
| 新能力、行为/架构/对外能力变化 | 先 OpenSpec，明确目标、兼容影响与验收；用户明确要求跳过 spec 时遵从并报告。 |
| 显式续作已有变更 | 读取指定 change 并核对现场，更新原记录，不另建同义项目。 |
| 既定迁移、运维或数据操作 | 核对环境、影响、已有授权与恢复措施，保留必要证据；流程文档不授予外部操作权限。 |

## OpenSpec 路由

先读 [config.yaml](../openspec/config.yaml) 的 context/rules/operations、相关 [主 spec](../openspec/specs) 和 [change](../openspec/changes)，并核对 [生成与废弃入口规则](02-engineering.md#生成与维护边界)。默认 `spec-driven`：proposal → specs → design → tasks，规划产物用中文；按实际已安装 skill 与 CLI 执行，不手建 change 目录，不把 skill 正文复制进产物。

| 动作 | 实际 skill |
| --- | --- |
| 探索需求 | [openspec-explore](../.agents/skills/openspec-explore/SKILL.md) |
| 逐步新建 / 一次生成实施产物 | [openspec-new-change](../.agents/skills/openspec-new-change/SKILL.md) / [openspec-ff-change](../.agents/skills/openspec-ff-change/SKILL.md) |
| 创建下一产物 | [openspec-continue-change](../.agents/skills/openspec-continue-change/SKILL.md) |
| 实施 / 验证 | [openspec-apply-change](../.agents/skills/openspec-apply-change/SKILL.md) / [openspec-verify-change](../.agents/skills/openspec-verify-change/SKILL.md) |
| 同步主 spec / 归档单个或多个 change | [openspec-sync-specs](../.agents/skills/openspec-sync-specs/SKILL.md) / [openspec-archive-change](../.agents/skills/openspec-archive-change/SKILL.md) / [openspec-bulk-archive-change](../.agents/skills/openspec-bulk-archive-change/SKILL.md) |
| 完整流程引导 | [openspec-onboard](../.agents/skills/openspec-onboard/SKILL.md) |

能力 id 是扁平 kebab-case，不用日程号；新能力先核对 config 的能力对照与 `openspec list --specs`，复用已有 id。用户点名其他入口时先定位实际 skill，缺失则报告，不编造 propose/update 等未安装入口。纯文档任务不会自动触发 OpenSpec 同步或归档。

主 spec 尚未覆盖全部已实施能力，相关 delta/实施记录仍在进行中 change。具体范围与例子只维护于 [开发工具事项](../docs/mod/dev-tooling/02-issues.md)，相关任务再对照代码、测试和该 change。tasks 勾完不等于主 spec 已同步或验收覆盖所有平台；治理接入不自动整理这些 change。

## Git 工作流

| 触发 | 读取与执行边界 |
| --- | --- |
| `coding begin` / `coding end`（含 `codeing`） | [git-team-workflow](../.agents/skills/git-team-workflow/SKILL.md)；只读取所需 Git 元数据，不加载业务/OpenSpec 上下文，不附带业务验证。 |
| `upd dev` | [git-upd-dev](../.agents/skills/git-upd-dev/SKILL.md)；仅按该显式口令运行同步流程。 |
| 进行中的 merge/rebase 冲突 | [resolving-merge-conflicts](../.agents/skills/resolving-merge-conflicts/SKILL.md)；由专用 skill 核对冲突意图并处理，不把同步 runner 当解冲突工具。 |

口令以外只做任务已授权的 Git 操作，不自行提交、推送、合并、改 config 或设计分支策略。需要提交时小步分组，不 force-push，不提交构建输出、`.pi`、密钥或会话用户数据。治理检查器只读，不能因文档里的命令执行 Git runner。

## 实施与验收

1. 核对已有差异和适用指令；已知目标直接进入代码与相关测试，不为普通任务预读整套手册。
2. 按证据 → 最小修改 → 定向验证 → 差异审查推进。命令、目录、前置依赖和副作用见 [项目手册](01-project.md)，测试/clippy 先读 README 安全章节并核实实际隔离。
3. 治理文档/检查器只跑只读框架、路径、链接、语法和差异检查；业务代码按 `make fmt` → 相关受限测试 → `make clippy`，收工 `make check`。GUI/Web 按实际影响补前端、Web、全界面或平台验收，不以默认终端检查代替。
4. 失败先区分回归、已有问题、环境缺失、参数或权限。输入/实现/环境/假设改变后再试；无新证据不反复运行，不删有效断言或绕过入口。继续可独立部分并报告具体缺口。
5. 同步变化的事实及真实未关闭事项，审查本次增量和新文件。已通过检查无相关变化不重复运行；工具退出 0、未配置外部连接和历史验收记录不等于本次业务/平台验收完成。

## 中断与交接

仅多阶段或中断任务保留短摘要：目标/验收、已授权与受限动作、相关分支及用户原有差异、本次文件、已运行验证/未验证原因、下一步与阻塞。优先现有会话或 change，不另造永久流水。恢复先核对最新现场，只重查失效结论；旧摘要不能扩大授权。
