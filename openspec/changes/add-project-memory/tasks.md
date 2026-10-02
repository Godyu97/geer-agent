# Tasks

## 1. 数据与项目上下文

- [x] 1.1 增加记忆记录、四种 DAO 后端的独立表/集合及共用服务，验证去重、编辑、排序、清空与重启持久化的受限 make test 单项。
- [x] 1.2 增加独立记忆开关、共用连接初始化和测试夹具隔离，验证配置组合及初始化失败单项。
- [x] 1.3 增加根项目指令刷新与原子切换/恢复，验证两种协议、热更新、缺失、错误与压缩后指令单项。

## 2. Agent 与公共操作

- [x] 2.1 注册自动执行的两个记忆工具并加入系统引导，验证顺序写搜、工具关闭、错误和有界输出的受限 make test 单项。
- [x] 2.2 增加公共记忆命令、结构化结果、状态与 REPL 确认，验证完整 CRUD、管道 --yes、失败可见和不触发模型的受限 make test 单项。

## 3. 管理界面

- [x] 3.1 增加图形快照及带版本的记忆删除/清空确认，验证客户端私有预览、过期拒绝和跨端同步的受限 make test FEATURES=web 单项。
- [x] 3.2 增加 TUI F4 管理面板、搜索/编辑输入及默认取消确认，验证键盘、窄屏、全文查看和聊天草稿保留单项。
- [x] 3.3 增加共用 React 记忆面板与状态展示，验证搜索、完整管理、确认、错误、忙碌、草稿和窄屏布局的 make frontend-test。

## 4. 集成与交付

- [x] 4.1 补充端到端根指令/记忆/重启与 Web 回归，更新 README、环境配置示例和架构说明；顺序完成 make web-test、make clippy-all、make check，并通过 openspec validate add-project-memory --strict。


## 验收记录（2026-10-02）

- 受限单项覆盖数据库记忆服务、两种 API 根指令热更新/压缩/恢复、自动写搜工具、REPL 管道确认、TUI 编辑与窄屏、Web 私有/过期确认。
- `make web-test`、`make clippy-all`、`make check`、`openspec validate add-project-memory --strict` 全部通过；前端 34 项通过。已有真实网页网络 smoke 测试按默认配置忽略。
- 最后样式修正后重新通过 `make frontend-test`、GUI/Web 类型检查和构建、受限 Web 构建及 `make clippy-all`。
- 浏览器实际以 375px CSS 视口验收：完整多行正文、总数更新、默认取消清空、确认清空及聊天草稿保留；面板边界完整。临时数据库、有限运行时服务与浏览器视口已回收/恢复。
- SQLite 已运行持久化与端到端回归；PostgreSQL/MySQL/MongoDB 适配已实现并编译，但当前未提供专用测试连接，未进行联机验收。
