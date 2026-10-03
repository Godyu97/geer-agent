# E1 Rust 社区教程调研：资料如何组成学习闭环

[返回总目录](../README.md) · [学习路线](../00-engineer-roadmap.md) · [下一篇](02-stack-decisions.md)

调查日期：2026-10-03。优先使用语言团队、课程作者、框架维护方的一手资料。推荐依据是内容覆盖与本学习目标的匹配，不按搜索排序、星数或付费营销判断教学质量。

## 主教材与练习

| 资料 | 维护/作者来源与定位 | 适合怎样使用 | 不能替代什么 |
| --- | --- | --- | --- |
| [The Rust Book](https://doc.rust-lang.org/book/) | Rust 官方，系统介绍语言 | 所有权、enum、trait、错误与并发的主教材 | 完整工程交付训练 |
| [Rust By Example](https://doc.rust-lang.org/rust-by-example/) | 官方，按主题展示代码 | 陌生语法快速查例子，修改输入验证 | 深层设计推理 |
| [Rustlings](https://github.com/rust-lang/rustlings) | rust-lang 维护的小练习 | 每学一主题就独立修一组练习 | 异步服务与数据库设计 |
| [Comprehensive Rust](https://google.github.io/comprehensive-rust/) | Google 课程，面向已有开发经验者 | 快速梳理基础，选择 async/领域模块 | 一遍课程后的长期熟练度 |
| [100 Exercises To Learn Rust](https://rust-exercises.com/100-exercises/) | 课程作者维护，逐步练习 | 用练习暴露所有权、类型与并发盲点 | 你的业务项目验收 |
| [Brown 交互版 Book](https://rust-book.cs.brown.edu/) | 教学研究团队增强版本 | 借助题目和可视化验证概念 | 当作独立的语言规范 |
| [Rust 语言圣经源码](https://github.com/sunface/rust-course) | 作者维护的中文课程 | 中文理解与章节回看 | 按版本核对官方规则 |

官方学习入口同时组织了 Book、RBE、Rustlings 与领域资料，见 [Learn Rust](https://rust-lang.org/learn/)。本仓库 [300 项映射](../00-course-map.md) 是固定历史源码映射，不声称实时覆盖原作者今天所有章节。

建议资深 Go 工程师选 Comprehensive Rust 或 Book 之一为主；Rustlings 或 100 Exercises 选一套主要练习。每个知识点仍由 std/Reference 查证，不要同时线性读完所有资源。

## 第二阶段教材

| 资料 | 主要方向 | 开始阅读的条件 | 配套任务 |
| --- | --- | --- | --- |
| [Rust for Rustaceans](https://rust-for-rustaceans.com/) | 类型、库设计与较深工程概念 | 能独立完成基础 CLI 和错误处理 | 审查自己的公共 API 与泛型选择 |
| [Rust Atomics and Locks](https://mara.nl/atomics/) | 原子、内存顺序和同步机制 | 理解 Mutex、Send/Sync 和线程 | 用图解释同步关系，不先写无锁队列 |
| [Zero To Production](https://www.zero2prod.com/) | 后端应用的实现与交付 | 熟悉 Tokio 和 HTTP 基础 | 完成配置、数据、测试和运行说明 |
| [Tokio Tutorial](https://tokio.rs/tokio/tutorial) | 任务、通道、I/O、framing 与 async | 所有权和线程概念已稳定 | 完成有界服务与优雅关闭 |
| [Command Line Book](https://rust-cli.github.io/book/) | CLI 工程 | 已会文件读取和 Result | 用 stdout/stderr/退出码完成一个工具 |
| [Rust Performance Book](https://nnethercote.github.io/perf-book/) | 性能定位与改进线索 | 已有可运行、可测量程序 | 一份可复现的优化记录 |

其中部分书为商业出版物，其公开作者页面用于核对定位，本次不声称阅读了付费全文，也不复制书中内容。是否购买根据方向与预算自行决定。

## 查询型资料，不建议从头背诵

- [std](https://doc.rust-lang.org/std/)：每次看签名、trait 实现、错误、panic 和版本标记。
- [Reference](https://doc.rust-lang.org/reference/)：确认语法、类型、借用等精确规则；它不是完整形式化规范。
- [Cargo Book](https://doc.rust-lang.org/cargo/)：解析、feature、manifest、profile、workspace 与构建工具。
- [Edition Guide](https://doc.rust-lang.org/edition-guide/)：判断历史代码与迁移语义。
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)：公共 API 审查清单。
- [Rustonomicon](https://doc.rust-lang.org/nomicon/)：学习 unsafe 边界的参考，不是新手实作前置。
- [Compiler Guide](https://rustc-dev-guide.rust-lang.org/)：需要解释编译器内部机制时按题查阅。

## 从源码学习的顺序

先看公共类型与 examples，再看测试，最后进入具体实现。问题可以具体到“谁持有连接”“什么时候释放许可”“哪个 await 允许取消”。跨平台 native 代码、unsafe、宏生成代码宜等问题明确后再深入。

适合本路线的阅读对象：Tokio mini-Redis 教程、Axum examples、Tauri 模板/项目 command、geer-agent 的 interaction/tools/session。没有必要一开始就通读整个 Tokio 或 Rust 编译器仓库。

## 论坛与博客怎么使用

[Rust Users Forum](https://users.rust-lang.org/) 适合带最小复现提问；[Rust Internals](https://internals.rust-lang.org/) 主要讨论语言与工具演进。提问写明版本、edition、完整诊断、最小代码、预期行为和已尝试的解释，去掉机密配置。

博客和 AI 答案可提供解释假说，但遇到“现在不支持”“永远”“自动取消”等说法，立即核对版本文档并写有限示例验证。社区调查也不能当作全行业统计：2025 State of Rust 调查在 2026-03-02 发布，报告收集 7,156 份完成回答并提醒解释样本范围。[官方调查](https://blog.rust-lang.org/2026/03/02/2025-State-Of-Rust-Survey-results/)。

## 资料阅读的成果模板

```text
主题与来源：具体章节、查阅日期、版本
我原来的理解：用自己的 Go 经验描述
新规则：一句可验证的结论
例子：成功版本、失败版本、输入边界
工程应用：项目中对应位置与取舍
尚未确认：需要实验或后续版本核对的点
```

读一章、解决一个设计问题、留下一个证据。相比收藏几十个链接，这更接近成为可协作的 Rust 开发者。
