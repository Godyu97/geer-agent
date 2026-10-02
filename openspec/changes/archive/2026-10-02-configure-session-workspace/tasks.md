# Tasks

## 1. Workspace 与会话状态

- [x] 1.1 在 session 层增加 `Workspace` 路径解析与规范化，并迁移现有会话状态/管理代码；用单元测试覆盖绝对、相对、`~/`、引号、中文空格、符号链接、同目录、文件和缺失路径
- [x] 1.2 让每条内存及持久化会话保存自己的 workspace，恢复时先校验目标目录，增加当前/全部范围的 DAO 列表；用会话单元测试覆盖跨目录往返、目录删除、保存失败补写和列表去重

## 2. Prompt、工具与 Agent 协调

- [x] 2.1 将 prompt 环境拆为一次异步探测和按 workspace 同步组装，并用双协议测试确认系统提示目录随新建/恢复会话更新且历史隔离
- [x] 2.2 让 Bash、文件及查询工具显式接收活动 workspace，更新工具说明；用两个临时目录的工具测试确认相对路径、默认查询根和 Bash cwd 都随会话切换
- [x] 2.3 在 Agent 中实现原子设置/恢复协调和授权重置，保持同目录、打开当前会话及失败路径不变；运行相关 Rust 单元测试验证 Session ID、授权和本轮用量行为

## 3. 公共命令与终端界面

- [x] 3.1 扩展 interaction 契约及解析器，接入 `/workspace [path]`、`/sessions --all`、状态和列表 workspace 字段；用解析和模拟 Session 测试验证 REPL/GUI 共用语义
- [x] 3.2 更新文本 REPL 的启动、切换、帮助和列表输出；用管道输入的集成测试验证查看、切换、新会话继承、无效路径及全部列表
- [x] 3.3 为 TUI 增加 workspace 状态显示和 F2 独立编辑模式；用渲染与事件测试覆盖 Enter、Esc、错误保留、聊天草稿及窄屏提示

## 4. 桌面 GUI

- [x] 4.1 在 GUI feature 中接入目录选择插件并限制为打开目录权限；运行 Cargo GUI 编译和前端类型检查验证依赖及 IPC 可用
- [x] 4.2 在 GUI 侧栏增加当前路径、手工输入、目录浏览、确认切换及当前/全部会话筛选；用前端测试覆盖取消、错误保留、忙碌禁用、快照更新和筛选

## 5. 集成验收

- [x] 5.1 用双协议模拟服务验证跨 workspace 的提示词、工具执行、会话保存与跨进程恢复，并运行 `cargo test` 确认默认构建回归通过
- [x] 5.2 运行 `cargo fmt --all`、`cargo clippy --all-targets`、GUI 前端 check/test/build、`cargo clippy --all-targets --features gui` 与 OpenSpec strict 校验，手工验收本机 REPL/TUI/GUI 并记录 Windows 目录选择未实测边界

### 2026-10-02 收工验收记录

- Windows 11 MSVC：`make fmt`、`make frontend-check`、`make frontend-build` 通过；`openspec validate configure-session-workspace --strict` 通过；`git diff --check` 通过。
- Windows 的 `make frontend-test` / `make clippy` / `make check` 按 Makefile 明确要求 Linux/systemd/cgroup 并失败退出，未绕过；同日 Fedora 受限 `make check`（251 项 Rust 测试通过、1 项忽略，clippy 无警告）与 `make frontend-test`（34 项）已覆盖同仓库实现。
- REPL（`GEER_AGENT_UI=repl` + release 通用程序）：`/workspace` 查看、切换临时目录、带空格目录、`/sessions` 与 `/sessions --all`、无效路径保持原 workspace 均通过。
- GUI：release `geer-agent-desktop.exe` 启动后出现标题为 `geer-agent` 的窗口（PE Subsystem=2）；系统目录选择器未在本机点击实测，记为未实测边界。
- TUI 交互式 F2 编辑未在本非交互会话手工操作；既有 TUI 单元/事件测试覆盖 F2、Enter、Esc、草稿与窄屏。
