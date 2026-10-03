# 类型系统进阶与内存布局：理解抽象的约束

[返回总目录](../README.md) · [API 设计](../03-engineering/09-api-design-and-state.md) · [Future 实验](15-future-send-and-pin-workshop.md) · [Trait 进阶](16-trait-boundaries-and-coherence.md)

本篇面向能读懂所有权、trait 和 async 的工程师。重点是识别何时需要进阶特性，以及它们承诺什么；主项目仍禁止 unsafe。

## HRTB：回调对任意短借用都能工作

```rust
fn apply<F>(callback: F) -> String
where
    F: for<'a> Fn(&'a str) -> &'a str,
{
    let local = String::from("Rust language");
    callback(&local).to_owned()
}

fn first_word(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or("")
}

fn main() {
    assert_eq!(apply(first_word), "Rust");
}
```

`for<'a>` 表示回调可以针对每次调用所需的生命周期成立，不是指定某一个贯穿整个结构的 `'a`。apply 内产生短期字符串，回调借出视图，返回前再建立拥有结果。[higher-ranked bounds](https://doc.rust-lang.org/reference/trait-bounds.html#higher-ranked-trait-bounds)。

## GAT：返回值借用本次 self

普通 Iterator 的 Item 对一个实现固定；需要返回与本次可变借用关联的视图时，GAT 可以让关联类型携带生命周期。

```rust
trait Lending {
    type Item<'a> where Self: 'a;
    fn next(&mut self) -> Option<Self::Item<'_>>;
}

struct Lines {
    lines: Vec<String>,
    index: usize,
}

impl Lending for Lines {
    type Item<'a> = &'a str where Self: 'a;
    fn next(&mut self) -> Option<Self::Item<'_>> {
        let line = self.lines.get(self.index)?;
        self.index += 1;
        Some(line.as_str())
    }
}

fn main() {
    let mut lines = Lines { lines: vec!["one".into(), "two".into()], index: 0 };
    assert_eq!(lines.next(), Some("one"));
    assert_eq!(lines.next(), Some("two"));
    assert_eq!(lines.next(), None);
}
```

返回视图的借用仍有效时，不能同时再次独占借用整个对象并继续使用旧视图。GAT 表达这个关系，不自动提供与 Iterator 全部适配器相同的组合能力。此例也可以直接用普通 slice iterator；它用于展示类型机制，真实代码不需重复实现遍历。[generic associated types](https://doc.rust-lang.org/reference/items/associated-items.html#generic-associated-types)。

## const generics：值参与类型身份

```rust
struct Window<const N: usize> {
    values: [u32; N],
}

impl<const N: usize> Window<N> {
    fn total(&self) -> u32 { self.values.iter().sum() }
}

fn main() {
    let window = Window { values: [1, 2, 3] };
    assert_eq!(window.total(), 6);
}
```

Window<3> 和 Window<4> 是不同类型；编译期长度能减少某些运行期长度检查，但动态输入仍需转换/验证。稳定 Rust 对可用于类型位置的常量表达式有具体限制，不应由这个例子推出所有 N+1 等表达式都直接支持。[const generics](https://doc.rust-lang.org/reference/items/generics.html#const-generics)。

## 借用型 API 的变型问题

共享引用与可变引用的类型替换能力不同。`&mut T` 对内部 T 的不变性，是为了防止通过可写位置塞入生命周期过短的引用。遇到需要把内层生命周期延长的报错，不能以“外层还在”或 transmute 处理；应重新定义拥有与借用关系。[subtyping and variance](https://doc.rust-lang.org/reference/subtyping.html)。

建议顺序：先画引用来源，再看谁能写入，再看谁会在什么时间读取。可变容器通常比只读视图更需要严格约束。

## 几种隐式转换各有条件

| 机制 | 典型例子 | 不能随意推广 |
| --- | --- | --- |
| Deref coercion | &String → &str | 任意拥有值不会因此自动转成另一拥有值 |
| unsizing coercion | &[T;N] → &[T]，具体实现引用 → dyn | 指针/trait 与布局条件仍受限制 |
| reborrow | 从 &mut T 临时再次借用 | 原独占使用在重借用有效时受约束 |
| numeric conversion | as 或 TryFrom | 不会自动保持范围与精度 |

coercion 发生在规定位置，并不等同于普通函数转换。审查数值转换时，先明确“溢出错误、截断、饱和或环绕”哪一种符合契约。[coercions](https://doc.rust-lang.org/reference/type-coercions.html)、[TryFrom](https://doc.rust-lang.org/std/convert/trait.TryFrom.html)。

## 布局要区分事实与保证

默认 repr(Rust) 允许编译器选择字段布局，不能把观察到的字节顺序作为外部协议。repr(C) 提供 C 兼容布局规则，但依然涉及 padding、alignment、目标平台和各字段本身的有效性；不是 JSON 或网络序列化机制。[type layout](https://doc.rust-lang.org/reference/type-layout.html)。

```rust
use std::{mem::size_of, num::NonZeroU32};

fn main() {
    assert_eq!(size_of::<Option<&u8>>(), size_of::<&u8>());
    assert_eq!(size_of::<Option<NonZeroU32>>(), size_of::<u32>());
}
```

这些类型有文档化的 Option 表示保证，不能推广到任意 struct、enum 或 Option<T>。`size_of` 不计算堆上文本、Vec 元素或 Arc 内部对象的总占用；指针大小不是对象总成本。[Option representation](https://doc.rust-lang.org/std/option/index.html#representation)、[size_of](https://doc.rust-lang.org/std/mem/fn.size_of.html)。

## 宏与泛型解决不同问题

泛型处理类型化行为；macro_rules 处理 token 模式并生成语法；derive/proc-macro 在编译期转换输入语法。宏不应掩盖错误传播、资源分配或外部执行。遇到框架宏，先找生成的 trait/函数契约，再看自动生成细节。[macro_rules](https://doc.rust-lang.org/reference/macros-by-example.html)、[proc-macro](https://doc.rust-lang.org/reference/procedural-macros.html)。

宏的卫生、匹配、作用域与 edition 有规则，不等于文本替换。应用只需要一两个 helper 时，普通函数往往更容易诊断；不要提前建立自定义 proc macro 平台。

## Unsafe/FFI 的学习边界

安全 Rust 依赖类型与库实现维持不变量；底层 unsafe 需要维护引用有效性、别名、初始化、线程安全等具体条件。safe 包装的接口应使外部调用者无法破坏这些条件；写出一个 unsafe block 不能代替证明。

Pin、布局、原子和 FFI 都可能碰到这些边界，但本项目学习实践优先安全标准接口；详细参考 [Rustonomicon](https://doc.rust-lang.org/nomicon/) 与 [既有 FFI 笔记](08-unsafe-and-ffi.md)。

验收：选择 GAT、HRTB、const generic 中的一项，分别展示它解决的接口问题和一个不值得引入它的简单场景；解释类型复杂度增加了什么维护成本。
