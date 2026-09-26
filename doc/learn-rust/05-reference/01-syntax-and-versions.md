# 35 语法速查、派生特征与版本差异

[返回总目录](../README.md) · [资料与验证](02-resources-and-verification.md)

对应原教程：附录中的关键字、运算符、表达式、派生 trait、prelude、版本说明与 1.58–1.89 的版本解读。历史版本是查阅材料，不是初学者必须按发行顺序学习的路线。

## 经常卡住的符号

| 写法 | 当前上下文下的常见意义 |
| --- | --- |
| `&value` / `&T` | 借用表达式 / 引用类型 |
| `&mut value` / `&mut T` | 独占借用 / 独占引用类型 |
| `*reference` | 解引用；类型位置的 `*const T` 则是裸指针 |
| `Type::method` | 关联项路径 |
| `value.method()` | 方法调用 |
| `::<u64>` | 显式传入泛型参数，常称 turbofish |
| `?` | 从 Result/Option 中取值，失败时提前返回 |
| `!` | 宏调用标记；单独作类型时是 never；作前缀运算时可取反 |
| `'a` / `'_` | 命名生命周期 / 让编译器推导的生命周期占位 |
| `..` / `..=` | 范围、剩余字段等，含义由位置决定 |
| `x @ pattern` | 匹配模式同时绑定整体值 |
| `\|x\| ...` | 闭包参数与函数体 |
| `where` | 展开泛型约束 |
| `move` | 闭包或异步块按值捕获环境 |
| `dyn Trait` | 特征对象类型 |
| `impl Trait` | 参数泛型约束或返回某个隐藏具体类型 |
| `r#name` | 原始标识符，可使用与关键字冲突的名字 |

同一符号要结合位置理解，不把类型语法、表达式语法和模式语法混在一起。`if`、`match`、普通块也能成为产出值的表达式；结尾分号经常是返回值变成 `()` 的原因。

## 常见 derive

| Trait | 提供什么 | 注意 |
| --- | --- | --- |
| `Debug` | 开发者调试表示 | 可能包含敏感字段 |
| `Clone` | 显式克隆 | 代价取决于字段类型 |
| `Copy` | 隐式按值复制的语义 | 需要字段满足 Copy，不能自定义 Drop |
| `Default` | 默认构造 | 默认值未必适合业务配置 |
| `PartialEq` / `Eq` | 相等比较与更强的等价承诺 | 浮点 NaN 是常见边界 |
| `PartialOrd` / `Ord` | 部分顺序 / 全序 | 默认派生顺序依赖字段/变体顺序 |
| `Hash` | 哈希输入能力 | 相等的值必须得到相同哈希 |
| `Serialize` / `Deserialize` | Serde 编解码 | 来自外部 crate，不是标准库派生 |

## prelude 不是所有标准库都自动导入

一些最常用的名称自动可用，例如 Option、Result、Vec、String；HashMap、Read、Write 等仍经常需要 use。edition 会影响 prelude 集合，遇到方法找不到也可能是提供方法的 trait 没在作用域中。原教程 prelude 条目只有标题，本段依据 [标准库 prelude](https://doc.rust-lang.org/std/prelude/index.html) 补充。

## 编译器版本、edition、crate 版本分别是什么

`rustc 1.x` 是编译器工具链版本；`edition = "2024"` 选择语言兼容规则；依赖如 Tokio `1` 是另一个库的版本系列。升级 edition 不是自动升级全部依赖，也不保证最新编译器支持的所有功能都存在于旧最低版本。

项目没有声明 `rust-version`，因此本次在本机编译例子通过，只能证明该环境可用，不能推断出准确 MSRV。`rustup show` 和 `rustc --version` 用于核实实际选中的工具链。

## 教程历史版本逐项速记

以下每行仅选取该篇讲到的一个主题，并非完整发布说明。完整原文入口在 [逐章对照](../00-course-map.md)。

| 版本 | 本教程对应的阅读重点 | 初学者怎么用 |
| --- | --- | --- |
| 1.58 | 格式字符串捕获变量 | 读懂 `format!("{name}")` |
| 1.59 | 解构式赋值、内联汇编 | 掌握解构，汇编选读 |
| 1.60 | Cargo 构建计时与 feature 语法 | 构建慢时查 `--timings` |
| 1.61 | 自定义 main 退出码 | CLI 可以明确表示失败 |
| 1.62 | cargo add、枚举默认变体 | 区分新增依赖和选择默认状态 |
| 1.63 | scoped threads | 在受控作用域中借用给线程 |
| 1.64 | IntoFuture、工具链组件 | 认识 await 的扩展点 |
| 1.65 | let-else、GAT | 先用 let-else 简化失败分支 |
| 1.66 | black_box、cargo remove | 基准与依赖管理按需查 |
| 1.67 | async fn 上的 must_use | 不忽略需要驱动的 Future |
| 1.68 | 稀疏索引、pin! | 不照搬很老的索引配置 |
| 1.69 | cargo fix 改进 | 自动修改后仍审查差异 |
| 1.70 | OnceLock、OnceCell、IsTerminal | 项目的终端判断属于此类能力 |
| 1.71 | C-unwind ABI | 跨语言边界时再深入 |
| 1.72 | cfg 禁用项的诊断 | 方法不存在可能是 feature 没开 |
| 1.73 | panic 信息、线程局部初始化 | 学会读取诊断来源 |
| 1.74 | Cargo 中配置 lint | 不照搬别人仓库的告警门槛 |
| 1.75 | 原生 trait async 与返回 impl Trait | 本项目 async trait 无需因旧文增添宏 |
| 1.76 | ABI 兼容性、引用类型名 | 低层兼容与调试选读 |
| 1.77 | 带间接层的 async 递归、offset_of | 递归 Future 仍需解决大小问题 |
| 1.78 | 诊断属性及 unsafe 前提检查 | 编译通过仍要理解接口前提 |
| 1.79 | 内联 const、临时值规则 | 有需求时查准确上下文 |
| 1.80 | LazyLock / LazyCell | 简单惰性初始化可用标准库 |
| 1.81 | expect lint、core::error::Error | 区分属性与 `Result::expect` 方法 |
| 1.82 | cargo info 等更新 | 查询 crate 信息，不猜 API |
| 1.83 | 更多 const 能力 | const 可用范围要按版本判断 |
| 1.84 | 依赖解析考虑 rust-version | 理解工具链兼容性约束 |
| 1.85 | Rust 2024、async 闭包 | edition 与语言特性分开核对 |
| 1.86 | trait upcasting | 动态接口转换仍要满足 trait 条件 |
| 1.87 | 匿名管道等系统接口 | 有真实进程通信需求再用 |
| 1.88 | let chains | 项目 `if ... && let ...` 需要相应支持 |
| 1.89 | const 泛型推断、生命周期语法诊断 | 目录虽简短，正文不是空白 |

本次目录的版本解读止于 1.89，不表示 Rust 的最新版本是 1.89。对新版本和历史细节以 [Rust 官方发布记录](https://github.com/rust-lang/rust/blob/master/RELEASES.md) 与 [Edition Guide](https://doc.rust-lang.org/edition-guide/) 为准。

## 项目最相关的迁移提醒

原生 trait async 已可用于静态分派，但 `ChatProvider` 仍不能直接构造成 dyn 对象；edition 2024 的环境修改 API 有新的 unsafe 边界；let chains 需要支持该特性的编译器。不要仅凭“用了 2024”就推断所有工具链版本都可构建项目。
