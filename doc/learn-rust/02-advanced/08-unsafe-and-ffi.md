# 19 Unsafe、FFI 与内联汇编：先认识边界

[返回总目录](../README.md) · [上一篇](07-globals.md) · [下一篇](09-macros.md)

对应原教程：Unsafe 简介、五种兵器、内联汇编。本章为选读；本项目禁止自行引入 unsafe。

## unsafe 并不会关闭所有 Rust 检查

它允许执行某些需要程序员额外保证前提的操作。类型检查和普通借用检查仍在工作；安全责任从编译器可以证明的部分，转移到作者必须证明的不变量。

教程列举的主要能力包括解引用裸指针、调用 unsafe 接口、访问可变静态项、实现 unsafe trait、读取 union 字段。学习时关注每种操作的前提：指针有效吗、对齐吗、对象初始化了吗、别名规则满足吗、会不会重复释放？

## 裸指针与引用的区别

引用必须满足有效性约束；裸指针可以表示空值等状态，创建一个裸指针与解引用它是不同动作。不要把 `*const T` 中的 `const` 理解成“数据永远不可能被别处修改”，它首先限制通过这个指针进行的操作。

`transmute`、任意地址转换、未初始化内存等不适合作为入门时绕开编译器的方法。需要两个不相交的可变切片时，先看安全的 `split_at_mut`。

```rust
fn main() {
    let mut values = [1, 2, 3, 4];
    let (left, right) = values.split_at_mut(2);
    left[0] = 10;
    right[0] = 30;
    assert_eq!(values, [10, 2, 30, 4]);
}
```

## FFI 与汇编额外增加什么问题

FFI 是跨语言边界，需要约定 ABI、数据布局、所有权、分配器以及错误/展开边界；`repr(C)` 只解决一部分布局问题，不自动保证传过去的对象有效。`extern` 声明也不表示外部函数天然安全。

`asm!` 还需要正确描述输入、输出、寄存器破坏和内存影响。初学阶段知道它用于底层场景即可；不在 Agent 项目里增加一个汇编演示。

## 旧教程与 edition 2024 的差异

2024 edition 将 `std::env::set_var`、`remove_var` 等调用标为 unsafe；旧例子即使在历史 edition 编译过，也不能原样假设适合本项目。读环境变量的 `std::env::var` 与为新进程设置环境的 `Command::env` 是不同接口。

Miri 能检测执行路径中的多种未定义行为，但测试通过不构成所有输入、所有并发执行的安全证明。链表篇将用它解释为什么底层容器代码需要更强验证。

来源：[教程 Unsafe](https://beatai.org/rust-course/advance/unsafe/intro)、[Rust Book Unsafe](https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html)、[edition 2024 newly unsafe functions](https://doc.rust-lang.org/edition-guide/rust-2024/newly-unsafe-functions.html)。
