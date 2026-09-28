# Tasks

## 1. 压缩地基

- [x] 1.1 增加 272,000 窗口、90% 触发及可选会话数据库配置；用配置单元测试验证默认值、覆盖值和无效配置。
- [x] 1.2 在 Prompt 增加完整轮次边界、摘要投影、原始事件与可恢复快照；用 Prompt 单元测试验证 Chat/Responses 工具配对、回滚与连续压缩。
- [x] 1.3 让两种 provider 的摘要请求返回完整文本和完成状态，并限制摘要输出；用模拟响应测试拒绝空白、截断及工具调用。
- [x] 1.4 在 Agent 中加入上下文估算、自动/手动摘要与资源记账；用 Agent 和模拟服务测试验证阈值、工具批次后压缩、失败保留原历史与一次溢出重试。

## 2. 会话存储与恢复

- [x] 2.1 在现有 DAO 增加固定名称的 SQL 迁移、会话事件及条件检查点存取；用 SQLite 契约测试验证追加、读取、revision 冲突与部分写入不可见。
- [x] 2.2 给 MongoDB 增加同一会话存取契约；用现有可选后端测试入口验证四种后端的共同行为，未配置的外部数据库明确标注未运行。
- [x] 2.3 连接 Agent 的检查点保存、补写与安全恢复；用 SQLite 加模拟模型测试验证正常恢复、工具中断、保存失败及不兼容会话拒绝。
- [x] 2.4 扩展 REPL 的 `/compact`、`/sessions`、`/resume <session-id>` 和 `/reset` 交互；用双协议 REPL 集成测试验证命令不进入历史、恢复后授权重置和 Session ID。

## 3. 验收与文档

- [x] 3.1 更新 README 与计划文档的配置、架构和失败边界；核对文档命令与实际 REPL 帮助一致。
- [x] 3.2 依次运行 `cargo fmt --all`、完整 `cargo test`、`cargo clippy --all-targets --all-features` 与 `openspec validate session-context-compaction --strict`，再以本地模拟服务做跨进程“压缩→退出→恢复→继续”验收。
