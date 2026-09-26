# 06 方法、泛型、Trait 与特征对象

[返回总目录](../README.md) · [上一篇](05-control-flow-and-patterns.md) · [下一篇](07-collections.md)

对应原教程：方法；泛型、特征、特征对象、进一步深入特征。

## `impl` 把行为放到类型旁边

```rust
struct Budget { remaining: usize }

impl Budget {
    fn new(remaining: usize) -> Self { Self { remaining } }
    fn remaining(&self) -> usize { self.remaining }
    fn consume(&mut self) { self.remaining = self.remaining.saturating_sub(1); }
    fn finish(self) -> usize { self.remaining }
}

fn main() {
    let mut budget = Budget::new(3);
    budget.consume();
    assert_eq!(budget.remaining(), 2);
    assert_eq!(budget.finish(), 2);
}
```

`new` 没有 `self`，是关联函数，名字本身没有特殊魔法。`&self` 读取实例，`&mut self` 修改实例，`self` 消耗实例。`Self` 指当前实现的类型。

## 泛型先提出要求，再让具体类型满足

```rust
trait Named {
    fn name(&self) -> &str;
}

struct ReadTool;
impl Named for ReadTool {
    fn name(&self) -> &str { "read" }
}

fn label<T: Named>(tool: &T) -> String {
    format!("工具：{}", tool.name())
}

fn main() {
    assert_eq!(label(&ReadTool), "工具：read");
}
```

`T` 不是任意可随便操作的对象；函数体只能使用约束承诺的能力。`where T: Named` 可以把较长约束移到签名下面。参数位置的 `impl Named` 也表达泛型约束；返回位置的 `impl Named` 隐藏的是某一个具体类型，不允许随分支任意返回不相关类型。

泛型可以用于结构体、枚举和方法。`const N: usize` 可把数组长度作为泛型参数，例如 `fn count<T, const N: usize>(_: &[T; N]) -> usize { N }`。编译期长度参数和运行时传入的普通数字不是同一件事。

## 三种分派方式

| 方式 | 何时决定具体实现 | 项目例子 |
| --- | --- | --- |
| `P: ChatProvider` | 编译时按具体类型生成代码 | 工具循环的 Provider 参数 |
| `enum` + `match` | 运行时选择有限几个分支 | `Provider::Api`、`TraceStore` |
| `dyn Trait` | 运行时通过特征对象调用 | `Box<dyn FnMut(...)>` 授权回调 |

```rust
trait Describe { fn describe(&self) -> &str; }
struct Read;
struct Write;
impl Describe for Read { fn describe(&self) -> &str { "read" } }
impl Describe for Write { fn describe(&self) -> &str { "write" } }

fn main() {
    let tools: Vec<Box<dyn Describe>> = vec![Box::new(Read), Box::new(Write)];
    assert_eq!(tools[1].describe(), "write");
}
```

`dyn` 表示通过接口使用具体对象；`Box` 提供拥有型指针。这是两个不同概念，`&dyn Describe` 也能动态分派，不必堆分配整个所有权包装。

## 不要直接把项目改成 `dyn ChatProvider`

[ChatProvider](/home/lihongyu/projects/geer-agent/src/provider/mod.rs) 的 `complete_step<F>` 既是泛型方法，又是原生 `async fn`。它的当前接口不满足直接构造特征对象的 dyn compatibility 要求。因此项目采用泛型参数配合内部枚举分派，并不是漏写了 `Box`。

原生 trait 中的 `async fn` 已稳定，不需要因为旧文章说“不支持”就添加 `async-trait`；但稳定支持静态调用，不代表自动支持 `dyn`。这是学习旧异步章节时必须校正的区别。

## 深入 Trait 时记住这些入口

关联类型如 `Iterator::Item` 表示“某个实现关联的元素类型”；泛型参数则可以允许同一个类型面向不同参数实现接口。超特征 `trait A: B` 要求实现 A 的类型也实现 B。`<Type as Trait>::method(...)` 用来消除同名方法歧义。

为外部类型实现外部 trait 受到孤儿规则限制。需要控制语义时，可以定义自己的包装类型，见 [newtype](../02-advanced/03-type-conversions.md)。标准 trait 优先通过 `derive` 或明确实现获得，不要靠继承树模拟业务。

练习：为 `WriteTool` 实现 `Named`，直接复用 `label`。再思考两个具体对象如何放进同一个 `Vec`：有限种类可以枚举，开放接口可以考虑 `dyn`。

来源：[教程泛型与 Trait](https://beatai.org/rust-course/basic/trait/intro)、[Rust Reference：dyn compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility)、[Rust 1.75 trait async](https://blog.rust-lang.org/2023/12/28/Rust-1.75.0/)。
