# 开发工具与测试隔离：当前实现

## 命令与构建编排

[Makefile](../../../Makefile) 区分默认 Cargo 终端构建与准备前端的 `gui,web` 通用构建；Windows 额外构建 `desktop-gui`，隔离缓存后复制桌面程序。前端入口冻结安装并依次构建两种 Vite 模式；`check` 递归顺序调用 fmt、安全验证、Rust 测试、clippy，不因外层 `make -j` 并发四个阶段。

依赖以 Cargo/Bun 清单与锁文件为准；配置键样例在 `.env.example`，运行数据与构建输出由 `.gitignore` 排除。根清单和安全脚本属于工程实现，不能用「非业务源码」或后缀过滤掩盖其归属。

## 测试启动、拒绝与回收

`make test` → [test-safe.sh](../../../scripts/test-safe.sh) → 独立 `geer-agent-test-*.service` → 内部预检 → 实际测试命令。外层清除 Bash 启动/远程会话条件，设置有限内存、任务和总时限、禁用 swap，并使用 `KillMode=control-group`、`OOMPolicy=kill`、`PrivateTmp=disconnected`。

内部从 `/proc/self/cgroup` 核实 memory/pids/swap 限制，比较 `/tmp`、`/var/tmp` 与上一层设备号并确认独立 tmpfs，核对三个临时目录变量为 `/tmp`；任何条件不满足都拒绝启动目标命令。`trap` 仅辅助停止本轮命名服务，异常回收由 systemd/cgroup 生命周期保证，不能替代或取消外层隔离。

[tests/support/mod.rs](../../../tests/support/mod.rs) 的 `command` 清理 Shell 钩子并默认关闭记忆，`run`/`run_with_timeout` 使用明确 stdin/EOF、有界输出和总时限。Unix 为子进程建进程组；主进程正常退出也清理后代，超时/错误回收主进程；Windows 尝试 taskkill 整棵树。默认 20 秒、stdout/stderr 各 4 MiB，仍须外层资源隔离兜底。

测试网络夹具使用本机临时端口与时限；修改隔离、Shell/PATH、进程或超时前必须读 [README 安全政策](../../../README.md#测试与系统安全)。不能裸跑 `test-safe-check.py`、测试二进制或危险复现。

## 仓库内工作流与治理维护

`.agents/skills/.openspec-target` 标记已安装的 agents 目标，OpenSpec skill 用 CLI 刷新；规则在 [config.yaml](../../../openspec/config.yaml)，不手改生成的 skill。Git runner 随其 skill 保存可恢复阶段状态于 Git 元数据路径，并在冲突时移交专用 skill；此处只说明来源，不自动运行或授权同步、提交和推送。

治理检查器位于 [.ai/scripts/check_governance.py](../../../.ai/scripts/check_governance.py)，只读检查框架与声明范围，不执行 Markdown/catalog 命令；日常维护不依赖全局 skill 路径，范围与局限见 [维护手册](../../../.ai/04-documentation.md)。

## 验证入口与缺口

业务安全验证从根目录依次执行 README 中的 `make test-safety`、缺失命令单项、`make test TEST_ARGS='--test process_safety'`、`make check`。安全控制服务为 256 MiB/64 任务/90 秒，探针各自 64 MiB/32 任务/2 秒，OOM 最多分配 96 MiB；它们顺序运行，不叠加额度。

本轮只静态核对命令、隔离/夹具流程及工作流来源，并运行治理检查；未启动业务测试服务、Git runner 或数据库。Windows/macOS 等环境还没有仓库内等效受限 Make 入口，现有入口明确失败或依赖 Linux/systemd，下一步见 [02](02-issues.md)。
