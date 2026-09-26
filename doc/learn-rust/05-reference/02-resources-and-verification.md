# 36 来源、学习资料与验证说明

[返回总目录](../README.md) · [章节对照](../00-course-map.md)

## 本次采用的来源

1. 用户指定的 [BeatAI 教程入口](https://beatai.org/rust-course/about-book)。本次直接读取返回 403，未取得网页正文。
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
rustdoc --test --edition 2024 doc/learn-rust/01-basics/03-ownership.md
```

需要从仓库根目录执行。普通 Rust 代码块应编译运行；`compile_fail` 代码块应被编译器拒绝；项目摘录使用 `ignore`，不会谎称它能脱离内部类型独立运行。包含 Cargo/网络命令的 shell 围栏是操作说明，不由 rustdoc 执行。

## 本次实际检查结果（2026-09-26）

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

## 验证范围的限制

本次是学习文档工作，不修改业务源码或依赖，不执行真实模型请求，也不把数据库外部契约、Windows 运行或完整应用集成测试标成已验证。源码导读证明的是当前实现如何组织，不能代替行为测试。

Mermaid 图面向支持 Mermaid 的 Markdown 预览。语法检查与某一具体 Markdown 插件的实际显示效果是不同层次；正文提供了不依赖图形的解释。
