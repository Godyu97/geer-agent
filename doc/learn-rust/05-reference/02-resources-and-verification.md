# 来源、学习资料与验证说明

[返回总目录](../README.md) · [章节对照](../00-course-map.md)

2026-10-03 扩充的调研依据见 [调研记录](03-research-log.md)，学习路线见 [工程师路线](../00-engineer-roadmap.md)。下面先保留初版来源与历史结果，避免将早期统计误当本轮验证。

## 初版采用的来源（2026-09-26 历史记录）

1. 用户指定的 [BeatAI 教程入口](https://beatai.org/rust-course/about-book)。当时直接读取返回 403，未取得网页正文。
2. 作者维护的 [rust-course 源码](https://github.com/sunface/rust-course)，固定提交 `ebe2d82437f621085b0cb6895edbd1283ed63cb1`。[关于本书源码](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/about-book.md) 内指向 BeatAI，确认是该教程的来源。按该提交下载了公开目录中全部 300 个 Markdown 条目，用于核对目录、章节主题及占位状态；正文学习笔记按知识主题归纳，不声称每个小节都有独立实现。
3. geer-agent 提交 `4f6a0a2` 的 Cargo.toml、src 与 tests，作为项目现状依据；未读取或引用真实 `.env` 内容。
4. Rust/Tokio 等作者或维护方文档，用于补充当前语言规则和实用技巧；各篇提供主题链接。

目录中原文写 TODO 的标题仍保留标记。有些 TODO 正文已有部分内容，有些没有 TODO 却只有标题或外链；对照表的状态分别记录，不能简单把整个性能或附录部分都当成已完稿。

## 遇到问题该查哪一份官方资料

| 问题 | 资料 | 使用方法 |
| --- | --- | --- |
| 不理解基础概念 | [The Rust Book](https://doc.rust-lang.org/book/) | 按主题读解释和例子 |
| 想看可运行的小例子 | [Rust By Example](https://doc.rust-lang.org/rust-by-example/) | 改一个输入验证自己的猜测 |
| 想做中文练习 | [Rust By Practice](https://practice-rust-zh.beatai.org/) | 先自己修复，再看参考答案 |
| 标准类型/方法怎么用 | [std 文档](https://doc.rust-lang.org/std/) | 看签名、错误、panic 条件与版本 |
| 需要精确语言规则 | [Rust Reference](https://doc.rust-lang.org/reference/) | 查语法和约束，不作为第一本教材 |
| 编译错误 | [错误索引](https://doc.rust-lang.org/error_codes/) | 按 E 编号查原因 |
| Cargo / feature / 构建 | [Cargo Book](https://doc.rust-lang.org/cargo/) | 查具体命令和配置字段 |
| edition 差异 | [Edition Guide](https://doc.rust-lang.org/edition-guide/) | 判断旧代码为什么不能直接搬用 |
| Tokio 异步系统 | [Tokio Tutorial](https://tokio.rs/tokio/tutorial) | 从任务和消息一路到关闭 |
| JSON 与自定义类型 | [Serde](https://serde.rs/) | 核对 derive 属性和数据表示 |
| 接口命名与设计 | [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) | 做公共接口时按需查询 |
| 本地不能安装环境 | [Rust Playground](https://play.rust-lang.org/) | 只粘贴公开、无敏感数据的练习 |

查询第三方 crate 时尽量选择与 Cargo.lock 一致的版本文档，而不是默认以 latest 为项目实际版本。网络教程里的“现在不支持”尤其需要按日期复核。

## 如何验证这些 Markdown 示例

标准库独立例子可以直接交给 rustdoc：

```bash
scripts/test-safe.sh rustdoc --test --edition 2024 doc/learn-rust/01-basics/03-ownership.md
```

需要从仓库根目录执行并确认隔离有效。普通 Rust 代码块应编译运行；compile_fail 应被拒绝；should_panic 是有意运行失败的例子；ignore 是项目片段或需要第三方练习包的框架示例，不能作为已验证成功的独立程序计数。Shell 围栏不由 rustdoc 执行。

可重复的全部文档检查：

```bash
python3 doc/learn-rust/verify.py
scripts/test-safe.sh python3 doc/learn-rust/verify.py --examples
scripts/test-safe.sh python3 doc/learn-rust/verify.py --frameworks
scripts/test-safe.sh python3 doc/learn-rust/verify.py --run-frameworks
scripts/test-safe.sh python3 doc/learn-rust/verify.py --tauri-commands
scripts/test-safe.sh python3 doc/learn-rust/verify.py --diagrams
```

verify.py 使用 Python 3.11+ 标准库。默认仅检查本地文件链接、代码围栏和数量；examples 先核对实际 cgroup/私有临时目录，再顺序执行文档测试；frameworks 在本轮私有目录生成十个练习 binary，直接依赖取当前 Cargo.lock 的精确版本，再离线解析并生成练习包自己的临时 lock；不声称全部传递依赖图与主项目完全相同。run-frameworks 运行八个有限、无需外部服务的完整程序。tauri-commands 单独编译长任务命令和 handler 注册模块，关闭 Tauri 默认 Wry feature，不创建窗口。共享 target 仅用于复用编译缓存，不修改主项目依赖。

diagrams 需要 Bun/Node，在私有目录安装 Mermaid 11 和 jsdom 并解析图，工具依赖不加入主项目。所有阶段顺序执行，不叠加多个完整测试组；依赖缓存缺失或隔离失败时先查明原因，不绕过。图解析只能证明语法，不等于具体 Markdown 预览器的视觉验收。

## 初版实际检查结果（2026-09-26 历史记录）

| 检查 | 结果 |
| --- | --- |
| 文档文件 | 38 个 Markdown 文件 |
| 原教程章节映射 | 300 / 300 个公开 Markdown 条目均有笔记入口 |
| 本地文件链接 | 目标路径全部存在，包括源码链接 |
| 普通 Rust 示例 | 54 个，均通过 rustdoc 编译与执行 |
| 预期编译失败示例 | 2 个，均按预期被编译器拒绝 |
| 项目上下文签名摘录 | 1 个明确标记 ignore，未当成独立程序验证 |
| Mermaid | 21 张图，均通过 Mermaid 11 的语法解析 |

Rust 示例使用本机 `rustc/rustdoc 1.98.1`、edition 2024；Mermaid 校验依赖放在临时目录，没有加入项目 Cargo 或前端依赖。语法解析不等于已经在用户的具体 Markdown 预览器中逐图目测。

## 本轮实际检查结果（2026-10-03）

本轮新增 42 篇 Markdown，学习目录从 38 篇扩展为 80 篇；保留原 300 项章节映射。另增加可重复执行的 verify.py，更新导航、入口现状与测试安全命令，并将 94 个仓库源码链接改为相对路径。

| 检查 | 本轮实际结果 |
| --- | --- |
| 本地文件链接与代码围栏 | 80 篇、889 个本地链接全部通过；无未闭合围栏 |
| 普通 Rust 代码块 | 90 个，通过 rustdoc 编译与执行 main |
| 预期编译失败 | 8 个，均按预期被拒绝 |
| 预期 panic | 1 个，按预期触发 |
| Rust 文档检查文件 | 43 篇包含上述可执行或编译失败块，全部通过 |
| 框架完整程序编译 | 10 个，通过 Cargo check |
| 框架有限程序执行 | 8 个，通过 Cargo run 和内置行为断言 |
| Tauri 长任务命令 | 1 个完整命令/状态/handler 注册模块，独立 library 编译通过 |
| Mermaid | 53 张，通过 Mermaid 11.17.2 语法解析 |
| 差异与空白 | git diff --check 通过；所有 Markdown 无行尾空白 |

普通 Rust 块中有明确说明“只构造并丢弃 Future”的类型实验；执行它们的 main 不表示推进了 async 块。compile_fail 验证被拒绝，不钉死每个版本完全相同的报错文本。

框架程序验证分别为：

| 练习 | 编译 | 实际运行范围 |
| --- | --- | --- |
| Tokio actor、JoinSet 上限 | 通过 | 两个有限完整程序通过 |
| Tokio 分帧与关闭 | 通过 | 分片、空帧、EOF、截断、超限、取消与 Drop 回收通过 |
| Axum 本机服务、请求契约 | 通过 | 请求契约程序运行通过；400/413/405/504 和正常 JSON 响应通过 |
| Serde、Reqwest | 通过 | Serde round-trip 运行通过；Reqwest 客户端仅编译，不请求真实上游 |
| SQLx SQLite | 通过 | 私有内存数据库的事务、找到/未找到与关闭通过 |
| thiserror/anyhow/tracing | 通过 | 类型分类、原因链、context、subscriber 和 async span 示例通过 |
| Tauri 长任务模块 | 通过 | 命令宏、状态类型、序列化与注册编译；不启动 WebView |

Axum 的长期运行服务未在本轮启动，端口/TCP 行为不由内存请求实验代替。SQLx 示例也不证明 Postgres/MySQL/SeaORM 全部契约。Tauri 计数器模板入口、前端片段、SeaORM 上下文片段、clap/Rayon 方向例子等仍按所在围栏声明其上下文，不能把 18 个 ignore 块全计为自动运行通过；其中 10 个完整程序和 1 个命令模块由单独框架流程验证。

本机 rustc/rustdoc 1.98.1，edition 2024，host 为 x86_64-unknown-linux-gnu。所有执行均通过 scripts/test-safe.sh，实际核对有限 cgroup 内存/进程/运行时限、禁止 swap 与私有 tmpfs；多个阶段顺序执行。框架最终编译/运行组耗时约 18.7 秒；最后的文档/Tauri/图解析组约 25.8 秒，均正常结束。本轮 Tauri 首次依赖编译的最高观测峰值约 2.1G，未超过 4G 额度，swap 为 0。

这些统计描述本次检查快照；文档或工具链更新后应重新运行适用阶段，以 verify.py 当前输出为准。

## 验证范围的限制

本次是学习文档工作，不修改业务源码或依赖，不执行真实模型请求，也不把数据库外部契约、Windows 运行或完整应用集成测试标成已验证。源码导读证明的是当前实现如何组织，不能代替行为测试。

Mermaid 图面向支持 Mermaid 的 Markdown 预览。语法检查与某一具体 Markdown 插件的实际显示效果是不同层次；正文提供了不依赖图形的解释。
