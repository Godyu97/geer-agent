# 26 Cargo 工程指南

[返回总目录](../README.md) · [上一篇](01-tests.md) · [下一篇](03-logging-and-tracing.md)

对应原教程：Cargo 使用指南的上手、基础与进阶全部小节。本篇是日常查表，不要求一次记住。

## 基础章节逐项归纳

| 原教程主题 | 应理解的关系 | 本项目怎么用 |
| --- | --- | --- |
| 为什么有 Cargo、下载构建 | 一套命令协调依赖解析和编译 | 从仓库根目录执行 Cargo |
| 添加依赖 | 版本要求写进 manifest，具体版本进入 lock | 先说明标准库为什么不够 |
| package 布局 | src、tests、examples、benches 各有角色 | 当前有 binary 与集成测试 |
| toml 与 lock | 允许的版本范围与已选版本不同 | 应保留应用的 Cargo.lock |
| 测试与 CI | 相同命令可放在自动环境 | 仍需准备系统工具和 feature 条件 |
| Cargo 缓存 | registry、Git、工具缓存 | 缓存缺失时 offline 不会下载 |
| build 缓存 | 编译产物通常在 target | 不提交 target，不先用 clean 治所有问题 |

`version = "1"` 或 `"0.42.0"` 通常是兼容版本范围要求，不是字面意义上只能选一个版本。需要确认构建用了什么，查看 lock 和依赖树。

```bash
cargo tree -e features
cargo tree -d
cargo metadata --no-deps --format-version 1
cargo build --locked
```

`--locked` 要求不修改 lock 的解析结果，不代表不访问网络；`--offline` 限制网络，但依赖必须已在缓存里。二者解决不同问题。

## 进阶章节逐项归纳

| 原教程主题 | 核心用途 | 初学阶段的注意点 |
| --- | --- | --- |
| 指定依赖项 | registry、Git、path 等来源 | 知道当前真正用了哪个来源 |
| 依赖覆盖 | `[patch]` 等用于验证修复 | 不是直接修改缓存源文件 |
| manifest | package、dependencies、features、targets 等 | 不照搬无关字段 |
| Cargo Target | lib/bin/test/example/bench 等构建目标 | 与目标平台 triple 是两种含义 |
| workspace | 多个 package 共用工程配置 | 当前单 crate 不需提前拆 workspace |
| features 及示例 | 编译时选择能力、依赖组合 | 通常按并集合并，不应假设互斥 |
| profiles | dev/release 等优化和调试设置 | 性能比较说明使用的 profile |
| config.toml | registry、target、linker 等环境配置 | 避免把个人路径变成项目通用要求 |
| 发布到 crates.io | 打包元数据、文件与版本 | 先检查包内容，不能带密钥 |
| build.rs 及示例 | 构建期代码生成、原生库编译/链接 | 会执行程序，需声明重跑条件 |

本项目没有因为需要读取一个配置就使用 build.rs；`include_str!` 是不同的编译期机制。

## Cargo.toml 中已经存在的依赖

[实际配置](/home/lihongyu/projects/geer-agent/Cargo.toml) 包含 Tokio、futures-util、Serde、async-openai、数据库驱动等。只把它们按职责理解：运行时、流与 Future 工具、序列化、模型协议、持久化。无需一开始学习所有 SDK 类型。

`default-features = false` 表示该依赖的这个声明不主动启用默认 features；其他依赖路径仍可能启用相同 crate 的 feature。`cargo tree -e features` 比凭一个 manifest 行判断最终开关更可靠。

## 项目的 embed-env 是很好的编译期例子

```toml
[features]
embed-env = []
```

这个空列表表示 feature 本身不额外启用依赖，但源码可以用 `#[cfg(feature = "embed-env")]` 控制是否编译相关项。启用后，[config](/home/lihongyu/projects/geer-agent/src/config/mod.rs) 会通过 `include_str!` 读取仓库 `.env`。

因此 `cargo clippy --all-targets --all-features` 也会触发读取；文件缺失会导致编译失败，文件存在则内容可能进入构建产物。不要为验证笔记而构建携带真实配置的程序。本次仅检查笔记，不执行该构建。

`cfg!` 与 `#[cfg]` 也不同：前者产生布尔值，两边代码通常仍需通过类型检查；后者在编译前选择是否保留对应项。

## 交叉编译要同时满足三层条件

先有目标平台的 Rust 标准库，再有合适的链接器/本地依赖，最后还要在目标操作系统验证运行。构建出 PE 文件或 ZIP 包只能证明产物格式，不能代替 Windows 上真实运行。

## 学习项目中的日常顺序

修改业务代码后依仓库约定执行 fmt、相关 test、clippy；不要添加 `-D warnings` 改变现有门槛。文档改动则验证链接、例子和图即可。需要升级依赖时单独处理并检查变化，不借学习笔记的机会批量更新。

来源：[教程 Cargo](https://beatai.org/rust-course/cargo/intro)、[Cargo manifest](https://doc.rust-lang.org/cargo/reference/manifest.html)、[Features](https://doc.rust-lang.org/cargo/reference/features.html)、[Build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)、[Profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)。
