# Tasks

## 1. 默认数据库

- [x] 1.1 增加唯一的公共数据库配置、默认 SQLite 和独立 Trace/会话开关，移除旧专用变量读取；以配置单元测试覆盖默认值、自定义数据库、关闭与无效输入。
- [x] 1.2 为两种记录复用一次数据库连接与迁移，并创建默认 SQLite 父目录、忽略其文件；用临时目录和 DAO 契约测试验证共库。

## 2. 会话状态与命令

- [x] 2.1 在 Agent 会话模块增加每会话完整状态和活动/暂存管理，接入现有工具循环；以双协议单元测试验证 A/B 历史、摘要、估算偏差及授权切换。
- [x] 2.2 增加 `/new`、`/open`、`/save` 并兼容 `/reset`、`/resume`，合并 `/sessions` 列表；用 REPL 命令和内存模式测试验证 UUID、重复打开、失败打开及列表标记。
- [x] 2.3 将保存成功、待补写、revision 冲突归到各会话，退出和 EOF 补写并报告未保存 ID；以模拟故障与 SQLite 冲突测试验证切换后补写及不覆盖。

## 3. 集成验收与文档

- [x] 3.1 在临时工作目录完成 Chat/Responses 双协议的“新建→切换→压缩→退出→恢复”模拟服务测试，验证默认数据库同时存会话与 Trace、关联 UUID 和跨进程继续。
- [x] 3.2 更新 README、REPL 帮助及技术方案落地记录；核对环境变量示例、Git 忽略规则和实际输出一致。
- [x] 3.3 顺序运行 `cargo fmt --all`、完整 `cargo test`、`cargo clippy --all-targets --all-features` 和 `openspec validate improve-session-isolation --strict`，记录外部数据库未实测范围。
