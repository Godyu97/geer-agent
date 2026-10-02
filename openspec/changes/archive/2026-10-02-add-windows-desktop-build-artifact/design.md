# Design

## Context

动机见 proposal.md。现有 Make 的 build/release 启用 gui,web，frontend-build 已准备两种静态页面；desktop-gui 启用 gui，并通过 Windows 子系统属性避免控制台，同时在运行时限制为 GUI。两类 Cargo 构建目前输出同名文件，不能仅追加一次编译就保留两个正确产物。本设计解决产物隔离与发布顺序，保留现有 Rust 路径。

## Goals / Non-Goals

**Goals:** Windows 一次 build/release 交付两个可执行文件，明确区分 profile，前端只准备一次，两类构建失败均让 Make 失败，桌面编译不覆盖通用程序。

**Non-Goals:** 不增加公开 Make 命令或参数组合，不扩展跨平台交叉编译、安装器或任意 Cargo target 配置支持，不改运行期界面选择。

## Decisions

### 1. 沿用两个构建入口，按宿主判断追加桌面产物

以 Windows 原生 Make 环境的 OS=Windows_NT 判断宿主。build/release 先准备共用前端并构建 gui,web 通用程序，仅 Windows 追加 desktop-gui 编译及产物复制。make run 仍只构建并运行通用程序。不增加 make debug，也不通过启动时 GEER_AGENT_UI 决定编译内容；Windows 子系统在链接时固定。

### 2. 分离 Cargo 输出，保持两份产物可靠

桌面构建使用独立的 Cargo target 目录，默认为 target/desktop-gui，避免后续编译或失败覆盖通用的 target/debug 或 target/release 产物。桌面编译只启用 desktop-gui，不启用 web、embed-env；成功后把对应 profile 的 geer-agent.exe 复制为通用 profile 目录中的 geer-agent-desktop.exe。Windows 原生 Make 明确使用 cmd.exe，禁用 AutoRun；复制使用内建 copy /Y，并将路径分隔符转换为反斜杠，不依赖 Git Bash 的 cp 或 PowerShell 别名。路径均加引号。若用户通过 CARGO_TARGET_DIR 调整根输出目录，两份输出与桌面缓存同步跟随该目录；默认无额外配置即使用 target。

按现有 Make 每个目标的配方顺序执行 Cargo 与复制，任何失败终止该目标。build/release 可共享前端依赖，但不让同一个 profile 的通用与桌面程序写入同名 Cargo 产物。旧桌面文件若仍存在，不代表当前失败构建成功，文档应明确以 Make 退出状态为准。

不采用同目录重复编译后备份、恢复通用程序的办法，避免失败路径遗留错误子系统程序；不新增第二个 Rust main 或复制业务模块，避免两个入口逐渐分歧。独立缓存增加磁盘占用，但方案直接、无需引入依赖。

### 3. 沿用教学实现与现有运行契约

该变更属于构建工具调整，不对应新的教程日程能力，不新增 crate。Rust 模块、所有权、Session、异步/流式处理与配置加载均不变。构建错误交给 Cargo/Make 非零状态；运行错误继续沿用通用终端错误和桌面窗口错误。默认 Cargo 仍是轻量终端配置。

### 4. 同步构建说明并验证 Windows 分支

help、README、AGENTS 与架构文档说明 Windows 双产物及两种界面的差别，删除“桌面构建必须手动覆盖通用产物”的日常使用指导，保留直接 Cargo 的特殊用途说明。已完成的历史 change 不改写，本变更记录对原统一构建方案的增量调整。

在 Linux 用 make -n 分别检查默认分支与 OS=Windows_NT 分支的 build/release 配方，并确认 run 不增加桌面编译。真实 Windows 验收核对双产物及 PE 子系统，分别双击两份桌面程序、检查配置失败可见性，再验证通用程序的终端和 GUI 选择。

### 5. 原生 Windows 与 Linux 的配方兼容性修复

Windows 使用 cmd 的 cd /D、空行输出和存在性检查后的 rmdir /S /Q；Linux 保留 POSIX 的 cd、echo 与 rm -rf。清理只处理 Cargo 产物和前端 dist。Cargo、Bun、Python 与递归 Make 的可执行路径统一加引号，支持安装目录带空格；参数仍由各配方独立传递。

测试、前端测试、Web 测试、隔离探针、clippy 和完整检查依赖 Linux systemd/cgroup；Windows 在入口返回明确错误，不尝试执行 .sh 或 POSIX 环境变量赋值，不提供无约束回退。fmt、fmt-check、doc 与构建入口仍可在两个平台运行。本次不新增 Windows 测试隔离实现。

验证使用 Windows 原生 Make 实际构建、双产物哈希与 PE 子系统检查，以及两平台命令展开检查；覆盖带空格的工具路径与输出目录、run 不增加桌面产物和 clean 的平台命令。Linux 通过原受限入口运行 make check；不能执行的检查明确记录，不用模拟 OS 的 dry-run 代替真实构建。

## Risks / Trade-offs

- [Windows 首次构建时间和缓存占用增加] → 独立缓存支持后续增量构建，前端仍仅准备一次。
- [错误 feature 或复制路径导致两份文件实际相同] → dry-run 核对 feature/profile，Windows 核对 PE 子系统与双击行为。
- [构建与窗口验收需要对应平台] → 分别记录原生构建、命令展开和窗口验收结果，不把模拟 OS 的 dry-run 当成真实 Windows 验证。
- [项目治理仍描述 desktop-gui 仅走直接 Cargo] → 同步日常构建约定，同时保留直接 Cargo 构建能力。

## Migration Plan

增量修改 Make 与说明后，先核对两个平台的 dry-run，再顺序使用现有安全入口完成相关检查；在 Windows 运行 build/release 做双产物验收。无数据迁移。回滚只需恢复构建配方与说明，新桌面缓存和额外可执行文件可随项目 clean 清理。
