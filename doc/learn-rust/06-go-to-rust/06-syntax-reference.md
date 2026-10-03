# G6 Go 开发者的 Rust 语法与惯用法索引

[返回总目录](../README.md) · [上一篇](05-migration-workshop.md)

这张表帮助定位陌生写法，不建议逐字翻译 Go 程序。每项都要结合参数的所有权和错误边界阅读。

| Go 概念或写法 | Rust 写法 | 易误解之处 |
| --- | --- | --- |
| `:=`、`var` | `let`、`let mut` | 默认不可变绑定；类型本身仍可提供内部可变性 |
| `const` | `const` | `static` 另表示具有静态存储期的项 |
| 多返回值 | 元组或 Result | 成功元组与错误分支可分别建模 |
| `if`、`switch` | `if`、`match` | Rust 可以用分支结果作为表达式 |
| `for range` | `for x in iterator` | iter/iter_mut/into_iter 决定借用或转交 |
| `[]T` | Vec<T>、&[T] | 拥有容器与借用视图是不同类型 |
| `map[K]V` | HashMap<K, V>、BTreeMap<K, V> | 查找通常返回 Option<&V> |
| `*T` | &T、&mut T、Box<T> 或裸指针 | 按语义选择，不能都视为普通指针 |
| `nil` | None | Option<T> 的变体，不是通用空指针 |
| `struct` 与方法 | struct + impl | self/&self/&mut self 决定接收者如何使用 |
| interface | trait + impl | 满足关系通常显式；dyn 另表示动态分派 |
| `error` | impl Error 或错误 enum | `?` 传播，不自动记录或重试 |
| defer | 作用域与 Drop | 可失败清理必须单独表达 |
| channel | std::sync::mpsc、tokio::sync 等 | 多生产/消费、有界和异步能力各不相同 |
| goroutine | 线程或 executor 任务 | async 调用不会自动 spawn |
| `fmt.Printf` | format!、println!、write! | 宏有格式与 trait 约束 |
| 包导出大写命名 | pub / pub(crate) / pub(super) | 可见性与名字大小写分开 |

Go 侧语言规则以 [Go 规范](https://go.dev/ref/spec) 为准；Rust 侧详见 [Reference 的表达式](https://doc.rust-lang.org/reference/expressions.html)、[类型](https://doc.rust-lang.org/reference/types.html) 与 [可见性](https://doc.rust-lang.org/reference/visibility-and-privacy.html)。

## 高频符号不要只读发音

| 符号 | 当前语境的含义 | 阅读动作 |
| --- | --- | --- |
| `&` / `&mut` | 借用引用；类型处写引用类型 | 找被借用的数据和使用范围 |
| `*value` | 解引用；也可能参与赋值 | 看 Deref/DerefMut 与目标类型 |
| `::` | 路径或关联项 | 区分模块、类型和 trait |
| `?` | 提前传播错误或缺失 | 看当前函数返回类型 |
| `!` | 宏调用；单独作为类型时是 never | 区分 `panic!()` 和 `-> !` |
| `'_` | 让编译器推断生命周期 | 不表示忽略有效性检查 |
| `'a` | 命名生命周期关系 | 找对应输入与输出借用 |
| `..` | 范围、结构更新或模式剩余 | 根据位置判断 |
| `=>` | match arm / 宏规则 | 左侧模式，右侧处理 |
| `where` | 泛型约束 | 逐个解释约束带来的能力 |
| `#[...]` | 属性 | 可能影响编译、派生、配置或测试 |

## 一段综合阅读练习

```rust
fn parse_numbers<'a>(lines: impl IntoIterator<Item = &'a str>)
    -> Result<Vec<u64>, std::num::ParseIntError>
{
    lines.into_iter().map(str::parse).collect()
}

fn main() {
    assert_eq!(parse_numbers(["1", "2", "3"]), Ok(vec![1, 2, 3]));
    assert!(parse_numbers(["1", "x"]).is_err());
}
```

阅读顺序：输入可转换为迭代器；每项借用文本；解析产生 Result；collect 根据函数目标类型聚合成“全部成功的 Vec 或一个错误”。这是 `FromIterator` 的实现约定，失败后不会继续收集全部错误；若业务需要全量错误，应选择另一种结果表示。[Result 的 FromIterator](https://doc.rust-lang.org/std/result/enum.Result.html#impl-FromIterator%3CResult%3CA,+E%3E%3E-for-Result%3CV,+E%3E)。

## 需要主动改掉的三个旧印象

1. Go 已有泛型，不能再用“Go 没有泛型”解释 Rust 差异；应比较约束、关联类型、trait 系统和编译策略。
2. Go 循环变量捕获行为与语言版本相关，Go 1.22 修改了相应规则；不要继续把旧示例当作当前所有 Go 程序行为。[Go 官方说明](https://go.dev/blog/loopvar-preview)。
3. Rust 的 async trait 方法、异步闭包与 let chains 是否可用，要看稳定版本、edition 和具体限制；可用不代表自动支持 dyn 或任意生命周期。[Edition Guide](https://doc.rust-lang.org/edition-guide/)、[版本速查](../05-reference/01-syntax-and-versions.md)。

验收：读一个真实函数签名，解释每个符号所表达的约束；然后将它改写成你熟悉的 Go 接口草图，指出草图中哪些约束只能靠约定维持。
