# Tasks

## 1. Windows 双产物构建

- [x] 1.1 在现有 build/release 配方中按 Windows 宿主追加隔离目录的 desktop-gui 编译与重命名复制，保留通用构建与前端依赖；通过 `make -n build OS=Windows_NT` 和 `make -n release OS=Windows_NT` 核对 feature、profile、复制路径和执行顺序，并核对带空格的 CARGO_TARGET_DIR 路径被正确引用。
- [x] 1.2 核对非 Windows 与运行入口的兼容性；通过 `make -n build`、`make -n release`、`make -n run OS=Windows_NT` 确认非 Windows 不追加桌面程序、run 仍只启动通用程序，且不存在 make debug 或公开桌面专用目标。

## 2. 使用说明与验收

- [x] 2.1 同步 help、README、AGENTS 与架构文档中的 Windows 默认双产物约定；通过 `make help` 和定向文本检查核对命令、debug/release 输出目录及通用/桌面功能说明一致。
- [x] 2.2 提供 Windows 手工验收步骤：分别执行 build/release，核对两份可执行文件及 PE 子系统，双击桌面程序确认 GUI 默认启动与无控制台，错误配置确认窗口显示失败原因，并检查通用终端与 ENV=gui 路径；若当前无 Windows 环境，在交付说明中明确真实 Windows 构建与窗口验收尚未执行。
- [x] 2.3 顺序运行 `make check` 完成格式、安全探针、受限 Rust 测试和 clippy，使用 `git diff --check` 检查补丁；若隔离入口失败，记录原因并停止，不绕过安全限制。

## 3. Windows 原生 Make 兼容性修复

- [x] 3.1 为 Windows 明确配置 cmd.exe 和原生命令，修复 build/release 的复制、clean 的清理及帮助空行；核对 Windows 原生 Make 的命令展开，并实际构建 release，检查双产物、复制哈希和 PE 子系统。
- [x] 3.2 引用工具与目录路径，保留 Linux 受限检查配方，在 Windows 检查入口返回明确平台要求；更新 README，通过两平台 dry-run 核对带空格的工具与 CARGO_TARGET_DIR、run、clean 及检查入口。
- [x] 3.3 顺序运行格式检查、Linux 受限 make check、补丁检查和 OpenSpec 校验；记录实际执行的平台、隔离限制及未执行的验收，隔离失败不绕过。

## 验证记录

- 2026-10-02，Linux 环境：Windows build/release 的 dry-run 与带空格 CARGO_TARGET_DIR 检查通过；非 Windows build/release 与 Windows run 的配方保持通用路径。
- `make help`、`git diff --check` 与 OpenSpec 校验通过。
- `make check` 通过：安全隔离探针通过，Rust 测试 239 项通过、1 项忽略，clippy 无警告；测试 cgroup 峰值内存 3.3G，swap 为 0。
- 真实 Windows 原生编译、PE 子系统和双击窗口验收尚未执行；README 提供手工验收步骤。

### 2026-10-02 原生 Make 兼容性复核

- Windows 原生 GNU Make 4.4.1：`make release` 成功构建并交付两份 exe；桌面复制前后的 SHA-256 一致，通用版 PE Subsystem 为 3（Windows CUI），桌面版为 2（Windows GUI）。链接器输出了生成导入库的提示警告，构建退出码为 0。
- Windows 的 `make help`、`make fmt`、`make fmt-check` 通过；`make check` 按预期报告 Linux/systemd/cgroup 要求并非零退出，没有启动测试。build/release/run/clean 和带空格、正反斜杠输出路径的命令展开正确。
- Fedora 上以相同 Git 提交的独立源码快照和本次 Makefile 运行，目录名包含空格；`make check` 通过，251 项 Rust 测试通过、1 项忽略，clippy 无警告。隔离/异常清理探针全部通过；测试服务实际启用 4 GiB 内存、无 swap、256 任务和 10 分钟上限，私有 tmpfs 隔离检查通过，服务结束后已回收。测试峰值 4 GiB，clippy 峰值 1.7 GiB。
- Linux 和 Windows 的 build/release/run/clean、带空格工具路径及受限检查配方均已做 dry-run；`git diff --check` 和 OpenSpec 严格校验通过。
- Fedora 带空格目录中的 `make frontend-test` 通过 34 项测试，递归 Make 和 Bun 均使用带空格的可执行路径；受限 `make build release` 成功构建 GUI/Web 前端及 Linux debug/release 通用程序，服务峰值 4 GiB、无 swap，结束后已回收。
- 本次未执行 Windows debug 的完整编译和 GUI 双击交互；clean 仅核对命令展开，未删除开发目录现有产物。
