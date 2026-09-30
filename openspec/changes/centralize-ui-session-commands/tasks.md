# Tasks

## 1. 共用会话执行

- [x] 1.1 在 interaction 增加 execute、结构化命令结果与操作错误，集中调用既有 Session；用受限 `make test TEST=interaction::` 验证流式顺序、回调失败、命令不混入对话及删除预览/确认/UUID 校验。

## 2. 界面迁移与接入

- [x] 2.1 将文本界面与颜色迁入 ui/repl，移除顶层模块，解析测试归 interaction；用受限 `make test TEST_ARGS='--test tool_loop'` 和 `make test TEST_ARGS='--test session_compaction'` 验证命令、文案、EOF 保存、workspace 与管道删除安全。
- [x] 2.2 将 GUI commands 变为共用执行结果的界面适配，关闭保存复用共用入口；用受限 `make test TEST=ui::gui_commands::` 验证流式、错误和删除，并检查 gui feature 的实际桥接编译。
- [x] 2.3 TUI 普通消息、会话命令及面板写操作接入共用执行；用受限 `make test TEST=ui::tui::` 验证状态、草稿、列表与删除面板行为。
- [x] 2.4 将 CLI 工具确认移入文本 UI，Tools 默认拒绝并由启动组合注入各界面确认；用受限 `make test TEST=noninteractive_bash_request_is_denied_without_running` 与现有授权/切换回归，核对三条启动路径。

## 3. 文档与完整验收

- [x] 3.1 同步 docs/design 架构文档、模块依赖治理与源码导读的迁移路径，区分已实施重构与后续事件优化；核对文档源码链接与 `rg` 搜索结果。
- [x] 3.2 顺序执行 `make check`、受限 gui feature 检查以及 `openspec validate centralize-ui-session-commands --strict`；确认资源与临时目录隔离生效、无新增 clippy 警告，并记录原生 GUI/TUI 手工验收范围。

## 验收记录

- `make check`：217 项测试通过，资源与私有临时目录隔离探针通过，默认 clippy 无警告。
- `make clippy-all`：受限构建 GUI 前端，实际 GUI bridge 与 gui feature 的全部 Rust target 编译检查通过，clippy 无警告。
- 共用入口、GUI 命令、TUI 面板状态与授权回归，以及 tool_loop/session_compaction 集成测试均按上述受限入口完成。
- 原生 GUI 窗口与真实 TUI 终端的键盘/鼠标交互尚未手工验收；本次使用界面状态/适配单元测试、真实二进制集成回归和 gui feature 编译检查，不把编译检查当作窗口交互验收。
