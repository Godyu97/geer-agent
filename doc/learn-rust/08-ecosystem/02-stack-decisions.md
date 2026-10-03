# E2 常用生态与技术选型：按问题组合，而非一次学完

[返回总目录](../README.md) · [上一篇](01-learning-resources.md) · [Tokio 深讲](03-tokio-runtime.md)

“热门常用”在这里表示社区中有成熟公开资料、值得工程师掌握的代表方案。本次没有构造市场份额、下载量或性能排名；官方文档说明能力，本教程给出适合本项目的学习取舍。

## 一张技术栈地图

| 层/方向 | 代表工具 | 学习入口 | 选择问题 |
| --- | --- | --- | --- |
| 异步运行时 | Tokio | [原理与任务](03-tokio-runtime.md) | I/O 规模、线程归属、取消和背压 |
| CPU 并行 | Rayon | [领域篇](10-domains-and-interoperability.md) | 是否可拆独立 CPU 工作 |
| HTTP 服务 | Axum/Tower、Actix Web | [Web 章节](05-axum-and-tower.md) | 中间件、状态模型、团队已有集成 |
| HTTP 客户端 | Reqwest；更底层可考虑 Hyper | [客户端章节](06-serde-and-reqwest.md) | 连接复用、TLS、代理、流与预算 |
| 序列化 | Serde/serde_json | [协议章节](06-serde-and-reqwest.md) | 格式、类型契约、兼容性 |
| 数据访问 | SQLx、SeaORM、Diesel、MongoDB driver | [数据章节](07-database-and-rpc.md) | SQL 控制、模型、迁移和运行边界 |
| RPC | Tonic/Prost | [RPC 章节](07-database-and-rpc.md) | 多语言契约、流、deadline 与生成成本 |
| 桌面 Web UI | Tauri 2 | [原理](08-tauri-architecture.md)、[实战](09-tauri-workshop.md) | 系统 WebView、IPC 与权限 |
| Rust GUI/TUI | egui/eframe、Iced、Ratatui/Crossterm | [领域篇](10-domains-and-interoperability.md) | UI 经验、渲染方式和平台要求 |
| CLI | clap | [领域篇](10-domains-and-interoperability.md) | 参数规模、帮助与子命令 |
| 应用/库错误 | anyhow / thiserror | [类型与错误](../06-go-to-rust/03-types-traits-and-errors.md)、[完整使用](14-errors-and-observability.md) | 是否要暴露稳定可处理类别 |
| 观测 | tracing / tracing-subscriber | [服务工程](../03-engineering/11-production-services.md)、[观测实战](14-errors-and-observability.md) | span、过滤、脱敏与指标 |
| 测试与治理 | nextest、proptest、audit、deny | [验证](../03-engineering/12-debugging-and-profiling.md)、[依赖](../03-engineering/10-dependencies-and-supply-chain.md) | 故障覆盖、隔离、许可证与已知问题 |
| 社区与贡献 | API Guidelines、RFC、维护方贡献流程 | [社区实践](../03-engineering/15-community-and-open-source.md)、[契约验证](../03-engineering/14-testing-and-contracts.md) | 阅读版本规则、复现、公共 API 与评审 |
| 互操作 | wasm-bindgen、PyO3、CXX | [领域篇](10-domains-and-interoperability.md) | ABI、宿主限制与所有权边界 |
| 嵌入式 | embedded-hal、Embassy | [领域篇](10-domains-and-interoperability.md) | no_std、硬件中断与执行环境 |

## 五套可学习的最小组合

| 目标 | 起步组合 | 暂时不加什么 |
| --- | --- | --- |
| 文本 CLI | std；参数复杂后 clap | async/数据库/GUI |
| I/O 客户端 | Tokio + Reqwest + Serde | 自己写 executor 或 HTTP 协议 |
| Web 服务 | Tokio + Axum + Serde；需要时 SQLx | 全套插件平台与多租户 |
| 桌面工具 | Tauri + 熟悉的前端 + 小 Rust 业务模块 | 第二套重复业务与存储模型 |
| CPU 数据处理 | std 基线；测出收益后 Rayon | 把所有计算丢到 Tokio spawn |

这是课程建议，不是强制架构。已有项目优先增量利用现有技术栈，先比较引入新框架是否真的解决一个当前问题。

## geer-agent 实际版本快照

从 2026-10-03 工作区 Cargo.lock 读取：

| crate | 锁定版本 | 当前用途 |
| --- | --- | --- |
| tokio | 1.53.1 | current-thread、时间、进程、I/O |
| axum | 0.8.9 | 可选 Web HTTP/WS |
| tower | 0.5.3 | 服务组合 |
| reqwest | 0.13.5 | HTTP 与网络能力 |
| serde | 1.0.229 | 数据类型与协议 |
| sea-orm | 2.0.3 | SQL 会话、Trace 与记忆 |
| sqlx | 0.9.0 | SeaORM 使用的底层 SQL 依赖 |
| tauri | 2.12.0 | 可选桌面壳 |
| ratatui | 0.30.2 | TUI |

这是项目选择，不是整个生态最新版本清单。查 API 优先选择 lock 对应版本；教程以兼容版本路线示范，独立练习应另有自己的 lock。[Cargo.toml](../../../Cargo.toml)、[Cargo.lock](../../../Cargo.lock)。

## 技术决策可以只写一页

```text
场景：输入规模、并发、目标平台、团队经验
必需保证：所有权、协议、取消、资源与交付
候选：两个足够的可行方案
验证：各自完成同一个最小能力，不只跑 benchmark
成本：依赖、编译、原生环境、运维、维护
决定：当前选什么，触发重新评估的条件是什么
```

不要求给每个工具写大型 ADR 系统。读者能理解为什么 std 不够、为什么此时引入该 crate，即达到本学习项目的设计要求。
