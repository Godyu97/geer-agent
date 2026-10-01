# Design

## Context

见 proposal.md。当前 `main` 只调用 `ui::run`，`gui` feature 仅纳入依赖；Windows 默认链接控制台子系统。`make gui` 通过 Cargo 从调用者终端运行。`PromptContext` 执行 Bash 和 cmd 版本探测，工具执行及清理也创建命令，均未设置 Windows 无窗口标志。GUI 工作线程已有 `StartupError` 与等待前端连接的失败路径。

## Goals / Non-Goals

保留单一二进制和现有终端构建，增加明确的桌面产物；不在运行后关闭或隐藏用户终端，不改进程组清理算法，不自动安装快捷方式。

## Decisions

1. 增加 `desktop-gui = ["gui"]` 编译特性，Windows 在启用它时使用 `windows_subsystem="windows"`，debug/release 均生效。桌面构建将 `auto` 解析为 GUI；显式终端模式通过 GUI 报错。保留普通 `gui` feature 的混合调试能力和无 feature 的终端构建。子系统由链接时决定，不能按运行时 `.env` 切换；仅按 `gui` 更改子系统会破坏现有 Windows 终端使用，运行后隐藏则会有闪烁，也可能误关调用者终端。
2. `make gui-build` 先构建前端再构建桌面 release 产物。Linux 提供 `Terminal=false`、直接执行桌面产物的启动器模板，文档指导替换绝对路径，不通过 Bash/Cargo/终端模拟器。开发用 `make gui` 保留原含义；已有终端不会被程序关闭。
3. `config` 增加后台命令工厂，返回拥有自身参数的 Tokio `Command`，Windows 固定 `CREATE_NO_WINDOW`（0x08000000），其它平台普通构造；所有生产探测、Bash 工具与清理入口复用。不新增 crate、不使用 unsafe；借助现有 Tokio 对 Windows 标志的安全封装。沿用管道、输出上限、超时、进程组与回收语义，不使用 `DETACHED_PROCESS` 改变进程关系。
4. 桌面 `ui::run` 捕获环境加载和模式错误，将拥有的 `String` 送入 GUI 工作线程，复用现有 `StartupError` 展示及关闭路径。Agent 初始化仍在工作线程执行，异步模型/流式逻辑不变。配置加载优先级不变。错误通过 `Result` 传播，显示错误不创建新的同步原生弹窗或依赖。
5. 依赖终端输入输出的进程集成测试不在 `desktop-gui` 构建运行，继续由普通构建的 `make check` 全量验证。桌面构建运行共用单元测试与进程安全测试，前端增加启动失败显示及禁止发送回归。

## Risks / Trade-offs

- 桌面模式无法承载 Windows 交互终端 → 提供明确独立构建选项，拒绝终端模式并提示普通构建。
- 仅移除父进程控制台仍有子进程闪烁 → 覆盖探测、执行和清理全部生产创建点；用户主动运行终端模拟器不属于自动控制台范围。
- Linux 桌面启动器需要实际路径 → 提供模板和替换步骤，不修改宿主桌面配置。
- 当前主机无法替代 Windows 真机视觉验收 → 受限运行回归并交叉编译检查，记录双击、后台执行与超时的真机验收步骤及未执行项。

## Migration Plan

现有命令继续有效；桌面用户改用 `make gui-build` 的产物。使用终端版本回退无需修改数据库或配置。问题原因和操作说明写入 `doc/plan/2026-10-01-gui-launch.md` 并更新 README。
