# 调研记录：来源、版本与结论边界

[返回总目录](../README.md) · [教程选择](../08-ecosystem/01-learning-resources.md) · [验证说明](02-resources-and-verification.md)

调查日期：2026-10-03。项目基线：工作区 HEAD 99e8690、edition 2024、rustc 1.98.1，锁版本见 [生态快照](../08-ecosystem/02-stack-decisions.md)。这是文档扩充依据，不是最低版本承诺，也不代表网页今后不会变化。

## 调研方法

语言、标准库和工具行为优先查 Rust 官方文档；Go 对照优先查 Go 官方文档；框架查维护方站点、版本 API 文档和作者仓库；学习资料查课程作者页面。资料推荐与技术取舍是本教程的判断，与引用的事实分开。

本次没有构造框架市场份额排名，没有访问真实模型 API、生产数据库或真实 .env，没有以下载/星数证明框架更好，也没有把商业教材公开简介说成已阅读付费全文。

## 语言与工具链的一手来源

| 主题 | 来源 | 主要核对点 |
| --- | --- | --- |
| 官方学习结构 | [Learn Rust](https://rust-lang.org/learn/) | Book、RBE、Rustlings、领域资料与 Reference 的定位 |
| 所有权与借用 | [ownership](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html)、[borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html) | 移动、借用与资源有效性 |
| trait 与闭包 | [traits](https://doc.rust-lang.org/reference/items/traits.html)、[closure](https://doc.rust-lang.org/reference/types/closure.html) | dyn compatibility、捕获与调用能力 |
| 泛型与生命周期 | [bounds](https://doc.rust-lang.org/reference/trait-bounds.html)、[associated items](https://doc.rust-lang.org/reference/items/associated-items.html) | HRTB、GAT 与关联类型 |
| 布局和转换 | [layout](https://doc.rust-lang.org/reference/type-layout.html)、[coercions](https://doc.rust-lang.org/reference/type-coercions.html) | 表示保证与隐式转换边界 |
| 标准库 | [std](https://doc.rust-lang.org/std/) 及各篇具体类型链接 | I/O、路径、文本、集合、时间、同步与错误 |
| 工具选择 | [rustup overrides](https://rust-lang.github.io/rustup/overrides.html) | 当前工具链与配置优先级 |
| 编译原理 | [compiler overview](https://rustc-dev-guide.rust-lang.org/overview.html)、[MIR](https://rustc-dev-guide.rust-lang.org/mir/index.html) | IR、借用分析、代码生成与单态化 |
| Cargo | [features](https://doc.rust-lang.org/cargo/reference/features.html)、[profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)、[build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html) | 构建配置与依赖代码执行 |
| resolver | [2024 resolver](https://doc.rust-lang.org/edition-guide/rust-2024/cargo-resolver.html) | resolver 3 与 virtual workspace |
| 工程规范 | [Style Guide](https://doc.rust-lang.org/style-guide/)、[API Guidelines](https://rust-lang.github.io/api-guidelines/)、[SemVer](https://doc.rust-lang.org/cargo/reference/semver.html) | 格式、接口约定与兼容变化 |

## Go 对照的一手来源

| 来源 | 用来核对什么 |
| --- | --- |
| [语言规范](https://go.dev/ref/spec) | 值、接口、类型和并发语法 |
| [内存模型](https://go.dev/ref/mem) | 同步关系与数据竞争 |
| [GC 指南](https://go.dev/doc/gc-guide) | 回收成本、存活数据与资源清理 |
| [slice 说明](https://go.dev/blog/slices-intro) | slice 描述与共享底层存储 |
| [字符串说明](https://go.dev/blog/strings) | bytes、runes 与编码 |
| [defer/panic/recover](https://go.dev/blog/defer-panic-and-recover) | 函数级清理与失败处理 |
| [context](https://pkg.go.dev/context) | 取消/期限传播的合作式接口 |
| [Go 1.22 loop variables](https://go.dev/blog/loopvar-preview) | 防止重复传播旧循环变量捕获结论 |

Rust/Go 的比较使用具体资源与执行模型，不能由“没有强制 GC”推导 Rust 对任何服务都更快。

## 框架与工程工具来源

| 范围 | 主要来源 | 核对问题 |
| --- | --- | --- |
| Tokio | [runtime](https://docs.rs/tokio/latest/tokio/runtime/)、[tutorial](https://tokio.rs/tokio/tutorial)、[select](https://docs.rs/tokio/latest/tokio/macro.select.html)、[shutdown](https://tokio.rs/tokio/topics/shutdown) | worker/driver、Send、取消、关闭 |
| Tauri 2 | [process model](https://v2.tauri.app/concept/process-model/)、[commands](https://v2.tauri.app/develop/calling-rust/)、[state](https://v2.tauri.app/develop/state-management/)、[capabilities](https://v2.tauri.app/security/capabilities/) | WebView、IPC、状态和权限 |
| Axum/Tower | [Axum](https://docs.rs/axum/latest/axum/)、[Service](https://docs.rs/tower/latest/tower/trait.Service.html)、[ServiceBuilder](https://docs.rs/tower/latest/tower/struct.ServiceBuilder.html) | extractor、错误和 layer 顺序 |
| Actix Web | [官方入口](https://actix.rs/) | 替代 HTTP 技术路线 |
| 序列化 | [Serde attributes](https://serde.rs/attributes.html)、[lifetimes](https://serde.rs/lifetimes.html) | 协议表示、缺字段与拥有结果 |
| HTTP 客户端 | [Reqwest](https://docs.rs/reqwest/latest/reqwest/) | Client、TLS、timeout、redirect |
| 数据 | [SQLx](https://docs.rs/sqlx/latest/sqlx/)、[SeaORM](https://www.sea-ql.org/SeaORM/docs/index/)、[Diesel](https://diesel.rs/) | SQL、模型、池、事务与构建检查 |
| RPC | [Tonic](https://docs.rs/tonic/latest/tonic/)、[Protobuf](https://protobuf.dev/programming-guides/proto3/) | 代码生成、schema 与错误 |
| CLI/TUI/CPU | [clap](https://docs.rs/clap/latest/clap/)、[Ratatui](https://ratatui.rs/)、[Rayon](https://docs.rs/rayon/latest/rayon/) | 参数、重绘与并行工作 |
| 观测 | [tracing](https://docs.rs/tracing/latest/tracing/)、[Performance Book](https://nnethercote.github.io/perf-book/) | 异步 span 与成本定位 |
| 治理与测试 | [RustSec](https://rustsec.org/)、[cargo-deny](https://embarkstudios.github.io/cargo-deny/)、[nextest](https://nexte.st/)、[Miri](https://github.com/rust-lang/miri) | 已知问题、策略和工具边界 |

更多方向的具体链接放在 [领域篇](../08-ecosystem/10-domains-and-interoperability.md)，避免把所有参考入口挤进主教程。

## 社区情况如何解读

深化练习还核对 [Future/Wake](https://doc.rust-lang.org/std/task/trait.Wake.html)、[Tokio duplex](https://docs.rs/tokio/latest/tokio/io/fn.duplex.html)、[Tower Timeout 实现](https://docs.rs/tower/latest/src/tower/timeout/mod.rs.html)、[Axum backpressure](https://docs.rs/axum/latest/axum/middleware/index.html#routing-to-servicesmiddleware-and-backpressure)，明确区分 poll_ready 等待、响应 Future 和 body 读取预算。

协作与验证补充依据 [Rust 官方社区入口](https://rust-lang.org/community/)、[RFC Book](https://rust-lang.github.io/rfcs/introduction.html)、[测试组织](https://doc.rust-lang.org/book/ch11-03-test-organization.html)、[proptest](https://docs.rs/proptest/latest/proptest/)、[trybuild](https://docs.rs/trybuild/latest/trybuild/)。这些资料用于核对流程和工具职责，不表示本项目已引入这些工具。

[2025 State of Rust Survey Results](https://blog.rust-lang.org/2026/03/02/2025-State-Of-Rust-Survey-results/) 于 2026-03-02 发布，调查发生在 2025-11/12，完成回答为 7,156。报告提到官方在线文档与源码学习、编译/资源成本和语言复杂度等问题，也提醒样本不能过度外推。

本教程由此采用“官方规则 + 练习 + 源码 + 测量”的组合；这项课程设计是我们的推论。调查不能证明某个框架在所有团队最受欢迎，或读完某套材料必定获得职位。

## 版本与验证边界

最后的进阶补充核对 [async trait/RPIT 稳定说明](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/)、[Borrow 契约](https://doc.rust-lang.org/std/borrow/trait.Borrow.html)、[宏](https://doc.rust-lang.org/reference/macros-by-example.html)、[条件编译](https://doc.rust-lang.org/reference/conditional-compilation.html)；错误与观测例子依据 thiserror/anyhow 作者文档和 tracing 的 instrument/subscriber API。样例以本机编译/运行证据确认，而不是只按网页示例推测。

`latest` 页面用于调研能力，实际工程查与 lock 相同版本。旧站点、迁移中的仓库和链接可能重定向；例如 Tonic 原仓库入口此次转到 grpc/grpc-rust，Atomics and Locks 作者站点转到 mara.nl。原中文章节映射仍是固定提交，不随本次调研改写历史统计。

独立标准库例子、框架编译、GUI 运行、跨平台分发与真实协议交互是不同验证层。每一层分别记录，见 [验证说明](02-resources-and-verification.md)。
