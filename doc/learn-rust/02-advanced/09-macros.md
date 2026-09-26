# 20 宏：先读懂，再考虑编写

[返回总目录](../README.md) · [上一篇](08-unsafe-and-ffi.md) · [下一篇](10-async-future-and-pin.md)

对应原教程：Macro 宏编程，包括声明宏、derive、属性宏和函数式过程宏。

## 函数处理值，宏可以生成语法

`println!` 支持不同数量的参数，`vec!` 生成集合构造代码；它们在编译阶段展开，不是运行时字符串替换。宏展开后的代码仍需要通过 Rust 检查。

| 形态 | 项目里能找到的例子 | 作用 |
| --- | --- | --- |
| 函数式宏调用 | `format!`、`json!`、`include_str!` | 生成表达式或其他语法 |
| derive 宏 | `#[derive(Serialize, Deserialize)]` | 按类型结构生成实现 |
| 属性宏 | `#[tokio::main]`、`#[tokio::test]` | 转换被标记的项 |
| 条件编译属性 | `#[cfg(test)]` | 决定是否编译某部分；不是运行时 if |

并非所有带 `!` 的宏都属于同一种实现机制；使用时先知道生成什么、需要哪些 trait。

## 一个小型声明宏

```rust
macro_rules! tagged {
    ($name:expr, $value:expr) => {
        format!("[{}] {}", $name, $value)
    };
}

fn main() {
    assert_eq!(tagged!("read", "done"), "[read] done");
}
```

`$name:expr` 捕获一个表达式；宏里的重复模式可处理变长输入。若普通函数已经能清楚表达任务，就先用函数，避免把错误信息、调试和跳转都变复杂。

## 本项目三个值得阅读的展开点

[main.rs](/home/lihongyu/projects/geer-agent/src/main.rs) 的 `#[tokio::main(flavor = "current_thread")]` 建立运行时驱动 async 主体，宏本身不会让所有同步操作都变成异步。

[TraceRecord](/home/lihongyu/projects/geer-agent/src/trace/mod.rs) 的 Serde derive 生成序列化与反序列化实现；字段命名和缺省规则通过 Serde 属性控制。`derive(Debug)` 则可能直接输出字段内容，不能对含密钥结构体随意使用调试打印。

[config](/home/lihongyu/projects/geer-agent/src/config/mod.rs) 中 `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"))` 在启用 feature 时于**编译期**读文件，之后是二进制内容的一部分。这和程序启动时读文件完全不同。

自定义过程宏通常需要单独的 proc-macro crate，复杂度明显高于普通函数。本项目目前不需要为了封装几行重复代码创建这样的工程。

来源：[教程宏](https://beatai.org/rust-course/advance/macro)、[Rust Reference 宏](https://doc.rust-lang.org/reference/macros.html)、[include_str!](https://doc.rust-lang.org/std/macro.include_str.html)。
