# 文件工具续接计划

## 接手时状态（2026-09-27）

工作区已有未完成的 `improve-file-tools-usability` 实现：三个独立查询入口 `ls/glob/rg`、共享 Bash 执行层，以及 `read/write/edit`、工具定义、提示词和部分测试改动。OpenSpec 的 13 项任务中，只有 1.1 和 1.3 标为完成；其余须按验收结果逐项确认。

接手时 `cargo test tools:: --quiet` 17/18 通过。唯一失败的缺少命令测试夹具产生 Bash 递归，最后误报超时；当时的异常进程已终止。完整测试、Clippy 和最终验收尚未运行。Agent 测试仍有按旧工具总数 5 编写的断言，新总数为 8。

## 实现与验证顺序

1. 修复缺少命令的隔离夹具。先单独确认它会快速、稳定地返回 127，且失败或超时后不遗留子进程；再运行 `cargo test tools::`，根据真实失败修正实现或断言。
2. 复核查询和文件操作：`ls` 使用现成 `ls -1Ap`，`glob` 使用 `rg --files --glob`，`rg` 搜索正文；用户路径和模式必须作为独立参数传入。覆盖无匹配/执行失败、路径基准、特殊字符、超时和截断；检查 `read` 完整结果 50 KiB 与续读偏移、`edit` 的重叠歧义和整批不落盘、`write/edit` 的真实 `changed`。
3. 验证逐工具授权、拒绝和 `/reset`、连续只读批次及写入屏障；专用查询不取得通用 Bash 授权。更新仍假定 5 个工具的测试，检查 Responses 与 Chat Completions 均声明 8 个工具、错误结果与调用标识配对。精简重复说明并记录工具定义大小。
4. 在隔离临时目录跑通“查询 → 读取 → 精确编辑 → 再读确认”，记录本机 Bash、`ls`、`rg` 版本及平台。依次运行 `cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features`、`openspec validate improve-file-tools-usability --strict`。确认行为和证据后再逐项更新 `tasks.md`。若已配置可用模型，另做受控 REPL 抽样，并与离线回归分别报告。

## 边界与完成标准

保留三个各司其职的模型入口，不新增 Cargo 依赖，不自行实现遍历、glob 或正则；保留 `read/write/edit` 原有参数语义、每工具首次授权和 provider 中立的定义。共享选择指引放在 `src/prompt/mod.rs`。本轮只完成 `improve-file-tools-usability`，主 `file-tools` spec 基线缺失，归档另行处理。

完成时应有全部 13 项任务的验证证据、通过的质量命令、离线闭环和两种协议回归；真实模型效果及其他平台未验证的范围须如实注明，不能以 mock 结果代替准确率结论。

## 实施结果（2026-09-27）

- 修复了缺少命令的测试夹具：清空其继承环境，在空 `PATH` 下先确认命令快速返回 127，再验证 `ls/glob/rg` 均报告依赖不可用。通用 Bash 的自定义可执行文件测试改用已存在的 `false`，避免临时脚本的 `Text file busy` 偶发失败。
- `ls/glob/rg` 使用当前 Bash 中的真实命令完成离线闭环。隔离目录测试涵盖带引号和命令替换符号的路径/模式、原生忽略规则、无匹配、缺少命令、非法正则、截断、独立授权、写入屏障，以及“查询 → 读取 → 歧义错误 → 扩大上下文编辑 → 再读”。`read` 分段拼接、BOM/CRLF、UTF-8、50 KiB 和 2000 行边界、相同内容不写盘及 Agent 修改计数均有回归。
- 两种模型接口均声明 8 个工具，参数定义一致；mock 验证了调用标识配对、参数错误、授权拒绝、关闭工具和 Finalization 后恢复。8 个工具定义的序列化大小为 **5359 字节**（压缩重复说明前为 5856 字节）。此数值是定义体积，不代表模型调用准确率。
- 质量命令按顺序通过：`cargo fmt --all`；`cargo test --quiet`（80 个单元测试、4 个重试集成测试、24 个工具循环集成测试，合计 108 个）；`cargo clippy --all-targets --all-features`（无警告）；`openspec validate improve-file-tools-usability --strict`（有效）。严格校验另有信息提示：主 `file-tools` spec 尚不存在，归档前须先建立原基线；本轮未归档。
- 本机验证环境为 Linux x86_64、GNU Bash 5.3.9、GNU coreutils `ls` 9.10、ripgrep 15.2.0。未在 Windows 或其他 Bash/命令版本上运行。
- 已配置模型的受控 REPL 抽样使用隔离临时目录，记录的工具序列与结果为：`ls → glob → read` 成功；`rg(files) → rg(content) → edit → read` 成功；`read → read(next_offset) → write` 成功且 `changed=false`；`edit(ambiguous_old_text)` 失败后 `read → edit → read` 恢复成功。未使用通用 Bash；最终文件内容已核对，临时目录已清理。该小样本只证明这些路径可用，不构成统计准确率评估。
