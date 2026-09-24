# Tasks

## 1. 标识与配置

- [x] 1.1 加入已论证的依赖、Trace 配置解析与校验；用配置单元测试及 `cargo check` 验证关闭、四种合法后端和无效值。
- [x] 1.2 定义 Trace 记录、采集状态及异步读写接口，并接入启动和 `/reset` 的 Session ID 显示；用单元测试和 `cargo run` 的 `/reset` 步骤验证 ID 生命周期。

## 2. 模型调用采集

- [x] 2.1 在两种 provider 请求与流式结果处采集完整请求、合并响应、部分内容和尝试次数；用现有模拟 SSE 测试验证两种接口及重试计数。
- [x] 2.2 在 Agent 的普通和最终回答模型步骤接入 Request ID、超时后记录及非阻断写入；用模拟 provider 验证工具循环、失败、超时仍各留一条记录，`cargo test` 通过。

## 3. 四种数据库

- [x] 3.1 实现 SQL 实体、版本化迁移和 SQLite/PostgreSQL/MySQL 的单条写入读取；用 SQLite 实库测试与三种 SQL 构建检查验证。
- [x] 3.2 实现 MongoDB 连接、索引及单条写入读取；用临时 MongoDB 实例完成读写验证，`cargo check` 通过。
- [x] 3.3 实现四后端原生批量写入、逐项核对、批量读取和 Session 游标分页；用统一 DAO 契约测试验证空批、缺失 ID、冲突、部分失败和排序。

## 4. 集成与文档

- [x] 4.1 更新 `.env.example`、README 与 SQLite 文件忽略规则；手动验证无数据库仍能对话、数据库不可达只告警、示例 SQLite 可查询且文档说明敏感内容。
- [x] 4.2 运行四后端临时实例的 DAO 契约测试、两种模型接口的集成测试、`cargo fmt --all`、`cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate llm-call-trace --strict`；修复本变更新引入的问题。
