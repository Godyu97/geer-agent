# Tasks

## 1. 检索与记忆读取

- [x] 1.1 增加 Unicode 分块、中文 bigram 与 BM25-lite 检索及受限来源格式，保留旧搜索；补充分词、排序、长文本、去重、预算、数据库更新与失败恢复测试，以 `make fmt`、`make test TEST=recall` 验证，并在调研文档记录现状、教程、pi/Codex 证据与取舍。

## 2. Agent 与 Prompt 集成

- [x] 2.1 增加自动召回开关和本轮参考消息，覆盖两种 API、续轮、压缩、恢复、故障和状态刷新；补充 Prompt 单测及本机模拟服务集成测试，更新 README 与架构，以 `make fmt`、相关受限 `make test` 和 `make clippy` 验证。

## 3. 完整验收

- [x] 3.1 执行 `openspec validate add-active-memory-recall --strict` 与顺序 `make check`；核对临时召回不落入会话快照/事件、无额外模型请求、无新增依赖，记录通过的验证和已知限制。
