# 13 闭包与迭代器：读懂函数式管道

[返回总目录](../README.md) · [上一篇](01-lifetimes-and-static.md) · [下一篇](03-type-conversions.md)

对应原教程：函数式编程、闭包、迭代器。

## 闭包是可以捕获环境的函数值

```rust
fn main() {
    let mut output = String::new();
    {
        let mut on_delta = |text: &str| output.push_str(text);
        on_delta("hello");
        on_delta(" Rust");
    }
    assert_eq!(output, "hello Rust");
}
```

`|text: &str| ...` 是闭包。它借用了外部 `output`，每次调用都修改它，因此需要以可变方式调用。

| 约束 | 对调用方式的承诺 |
| --- | --- |
| `FnOnce` | 可以调用一次，允许调用时移出捕获的值 |
| `FnMut` | 可以反复调用，允许修改捕获环境 |
| `Fn` | 可以通过共享引用反复调用 |

支持 `Fn` 的闭包也支持 `FnMut`、`FnOnce`；支持 `FnMut` 的也支持 `FnOnce`。`move` 决定如何捕获，闭包体如何使用捕获值才决定实现哪些调用 trait；不要看到 `move` 就断言只支持 `FnOnce`。

[ChatProvider::complete_step](/home/lihongyu/projects/geer-agent/src/provider/mod.rs) 要求 `F: FnMut(&str) -> io::Result<()>`：每次流式片段到达都能调用回调，回调可更新输出状态，终端写失败还能返回错误。

## 迭代器是一条待执行的计算描述

```rust
fn main() {
    let lines = ["read a", "", "write b", "read c"];
    let reads: Vec<_> = lines.iter()
        .copied()
        .filter(|line| line.starts_with("read"))
        .collect();
    assert_eq!(reads, vec!["read a", "read c"]);
}
```

`map`、`filter` 一般只建立适配器；`collect`、`sum`、`count`、`for` 等消费它才产生遍历效果。`map` 适合转换值，单纯做副作用时 `for` 通常更直观。

```mermaid
flowchart LR
    A[源集合] --> B[iter 借用元素]
    B --> C[filter 判断保留]
    C --> D[map 转换]
    D --> E[collect 消费并收集]
```

## 项目常见组合

| 组合 | 含义 |
| --- | --- |
| `filter_map` | 每项可产出一个结果，也可跳过 |
| `find_map` | 找到第一个 `Some` 就停止 |
| `enumerate` | 附带从 0 开始的索引 |
| `zip` | 按位置配对，到较短一侧结束 |
| `cloned` / `copied` | 从引用产生拥有值或 Copy 值 |
| `collect::<Result<Vec<_>, E>>()` | 收集成功值，遇错提前返回 |

```rust
fn main() -> Result<(), std::num::ParseIntError> {
    let values = ["10", "20", "30"];
    let numbers = values.iter().map(|s| s.parse::<u64>())
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(numbers, vec![10, 20, 30]);
    Ok(())
}
```

[Responses::complete_step](/home/lihongyu/projects/geer-agent/src/provider/openai/responses.rs) 用 `filter_map` 只留下工具调用项；[parse_pretty_name](/home/lihongyu/projects/geer-agent/src/prompt/mod.rs) 用 `find_map` 找系统版本；[DAO](/home/lihongyu/projects/geer-agent/src/dao/sql.rs) 用 `collect` 合并一批可失败的转换。

易错点：`filter` 的谓词收到的是元素的引用，因此源迭代器已经产出引用时，闭包参数可能是双层引用。先写出 `Iterator::Item` 类型，再决定是否用 `copied`，不要连续加星号碰运气。

练习：将 `["1", "bad", "3"]` 解析成 `Result<Vec<u64>, _>`，应得到 `Err`。如果改为 `filter_map(|s| s.parse().ok())`，错误会被悄悄丢弃；两种写法表达的是不同需求。

来源：[教程闭包](https://beatai.org/rust-course/advance/functional-programing/closure)、[教程迭代器](https://beatai.org/rust-course/advance/functional-programing/iterator)、[Iterator 文档](https://doc.rust-lang.org/std/iter/trait.Iterator.html)。
