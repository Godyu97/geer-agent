# Tasks

## 1. Windows 双产物构建

- [x] 1.1 在现有 build/release 配方中按 Windows 宿主追加隔离目录的 desktop-gui 编译与重命名复制，保留通用构建与前端依赖；通过 `make -n build OS=Windows_NT` 和 `make -n release OS=Windows_NT` 核对 feature、profile、复制路径和执行顺序，并核对带空格的 CARGO_TARGET_DIR 路径被正确引用。
- [x] 1.2 核对非 Windows 与运行入口的兼容性；通过 `make -n build`、`make -n release`、`make -n run OS=Windows_NT` 确认非 Windows 不追加桌面程序、run 仍只启动通用程序，且不存在 make debug 或公开桌面专用目标。

## 2. 使用说明与验收

- [x] 2.1 同步 help、README、AGENTS 与架构文档中的 Windows 默认双产物约定；通过 `make help` 和定向文本检查核对命令、debug/release 输出目录及通用/桌面功能说明一致。
- [x] 2.2 提供 Windows 手工验收步骤：分别执行 build/release，核对两份可执行文件及 PE 子系统，双击桌面程序确认 GUI 默认启动与无控制台，错误配置确认窗口显示失败原因，并检查通用终端与 ENV=gui 路径；若当前无 Windows 环境，在交付说明中明确真实 Windows 构建与窗口验收尚未执行。
- [x] 2.3 顺序运行 `make check` 完成格式、安全探针、受限 Rust 测试和 clippy，使用 `git diff --check` 检查补丁；若隔离入口失败，记录原因并停止，不绕过安全限制。

## 验证记录

- 2026-10-02，Linux 环境：Windows build/release 的 dry-run 与带空格 CARGO_TARGET_DIR 检查通过；非 Windows build/release 与 Windows run 的配方保持通用路径。
- `make help`、`git diff --check` 与 OpenSpec 校验通过。
- `make check` 通过：安全隔离探针通过，Rust 测试 239 项通过、1 项忽略，clippy 无警告；测试 cgroup 峰值内存 3.3G，swap 为 0。
- 真实 Windows 原生编译、PE 子系统和双击窗口验收尚未执行；README 提供手工验收步骤。
