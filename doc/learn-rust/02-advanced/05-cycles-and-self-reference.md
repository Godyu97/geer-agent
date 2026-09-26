# 16 循环引用、自引用与 Weak

[返回总目录](../README.md) · [上一篇](04-smart-pointers.md) · [下一篇](06-threads-and-synchronization.md)

对应原教程：循环引用与自引用、Weak 与循环引用、结构体中的自引用。

## 循环强引用会让计数降不到零

若 A 用 `Rc` 拥有 B，B 又用 `Rc` 拥有 A，外部句柄都释放后，两者仍互相保持强引用。这可能泄漏内存；安全 Rust 并不保证没有资源泄漏。

```mermaid
flowchart LR
    A[父节点] -->|强引用拥有| B[子节点]
    B -. Weak 回看父节点 .-> A
```

通常把“拥有孩子”与“回看父节点”分开：回看使用 `Weak`，不阻止对象被释放。访问前 `upgrade()`，得到 `Some(Rc<_>)` 或 `None`。

```rust
use std::rc::Rc;

fn main() {
    let owner = Rc::new(String::from("session"));
    let observer = Rc::downgrade(&owner);
    assert!(observer.upgrade().is_some());
    drop(owner);
    assert!(observer.upgrade().is_none());
}
```

“弱引用”是观察所有者是否还在的句柄，不是可以随意解引用的失效指针。Arc 也有对应的 Weak。

## 自引用的难点是数据和地址之间的关系

假设结构体同时保存一个 `String` 和一个指向其内容的 `&str`。构造时如何建立借用、移动结构体是否影响被指位置、字符串重新分配后引用是否有效、销毁顺序如何保证，都会变成必须解决的问题。

对 `String` 而言，移动拥有者通常不移动堆缓冲区，但修改/扩容可能移动缓冲区；因此也不能只靠“堆地址暂时不变”就证明整个设计正确。

## 优先用范围、索引或独立拥有值

```rust
use std::ops::Range;

struct Command {
    source: String,
    name: Range<usize>,
}

impl Command {
    fn name(&self) -> Option<&str> {
        self.source.get(self.name.clone())
    }
}

fn main() {
    let command = Command { source: "read file".to_owned(), name: 0..4 };
    assert_eq!(command.name(), Some("read"));
}
```

范围在访问时重新转成借用；若修改源文本，仍要维护范围的语义，这个例子没有声称自动追踪编辑位置。

项目 [Prompt](/home/lihongyu/projects/geer-agent/src/prompt/conversation.rs) 保存拥有型会话项，没有在同一个结构体里保存指向自己字段的借用；[TraceContext](/home/lihongyu/projects/geer-agent/src/agent/mod.rs) 是独立的短期视图，借用外部 Agent。先用这种清楚的拥有者/视图分离，再考虑复杂自引用抽象。

`Pin` 与地址稳定有关，但不是写一个 `Pin<...>` 就能让任意自引用结构体自动安全；它的具体保证见 [异步与 Pin](10-async-future-and-pin.md)。

来源：[教程循环引用](https://beatai.org/rust-course/advance/circle-self-ref/circle-reference)、[教程自引用](https://beatai.org/rust-course/advance/circle-self-ref/self-referential)、[标准库 Weak](https://doc.rust-lang.org/std/rc/struct.Weak.html)。
