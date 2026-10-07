# 持久化适配：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 存储路由与失败边界

配置创建 SessionStore、TraceStore、MemoryStore，适配选择 SQLite/PostgreSQL/MySQL 或 MongoDB。SQL 通过 SeaORM migration 建立会话、Trace 和记忆结构；MongoDB 使用对应集合。上层 session、trace、memory 调用异步接口，数据库错误作为 TraceError/Result 返回，不伪装为空数据或易失性写入成功。会话保存使用 revision 检测并发冲突。

## 验证入口与缺口

代表性测试：[src/dao/mod.rs](../../../src/dao/mod.rs)、[tests/session_compaction.rs](../../../tests/session_compaction.rs)、[src/memory/tests.rs](../../../src/memory/tests.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

src/dao 内有适配测试；tests/session_compaction.rs 验证持久化与重启恢复，src/memory/tests.rs 覆盖记忆数据库失败。本次未运行。

`src/dao/mod.rs` 的 `external_contracts` / `external_session_contracts` 只在配置专用 `GEER_AGENT_TEST_POSTGRES_URL`、`GEER_AGENT_TEST_MYSQL_URL`、`GEER_AGENT_TEST_MONGODB_URL` 时调用对应后端；未配置时不做外部联机验证。不能将默认测试的成功状态或代码编译通过称为四种后端全部验收，现有联机缺口见 [02](02-issues.md)。

源代码入口：[src/dao](../../../src/dao)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
