# 12 深入生命周期与 `'static`

[返回总目录](../README.md) · [下一篇](02-closures-and-iterators.md)

对应原教程：高级进阶的生命周期、深入生命周期、`&'static` 和 `T: 'static`。

## 先区分三种不同的范围

作用域回答“名字在哪里可见”；值的生存期回答“数据什么时候被销毁”；借用生命周期回答“引用允许在哪里被使用”。它们相关，但不必完全一致。NLL 缩短借用，不会自动把拥有者也提前释放。

返回引用时，应只约束真正提供该引用的输入，不要习惯性把所有参数绑到同一个 `'a`。

```rust
fn preferred<'a>(name: &'a str, _fallback: &str) -> &'a str {
    name
}

fn main() {
    let name = String::from("read");
    let selected;
    {
        let temporary = String::from("fallback");
        selected = preferred(&name, &temporary);
    }
    assert_eq!(selected, "read");
}
```

如果返回值事实上不会借用 `fallback`，就没有必要让短命的 fallback 限制结果。类似地，方法写成 `fn get(&self) -> &str` 往往已经够用；把接收者写成 `&'a mut self` 并绑定结构体的大生命周期，可能让借用范围过大。

## 两种 static 不能混为一谈

| 写法 | 真正的含义 | 不代表什么 |
| --- | --- | --- |
| `&'static str` | 所引用的文本可以在整个程序期间有效 | 所有字符串引用都必须如此 |
| `T: 'static` | T 不含限制其存活范围的短期借用 | T 必须活到进程结束 |

拥有自身数据的 `String` 可以满足 `T: 'static`，但仍可在局部块结束时释放。`move` 闭包取得捕获值的所有权；如果捕获的值本身是一个短期引用，`move` 不会把它变成 `'static`。

```rust
fn require_static<T: 'static>(value: T) -> T { value }

fn main() {
    let owned = String::from("request");
    let owned = require_static(owned);
    assert_eq!(owned, "request");
    drop(owned);
}
```

## 特征对象的隐含生命周期

普通类型位置的 `Box<dyn Trait>` 经常默认要求内部对象满足 `'static`。若它要借用调用者的数据，可能需要 `Box<dyn Trait + 'a>`；引用和具体表达式上下文也会影响默认推导，不能把“所有 dyn 永远默认 static”当作统一规则。

项目 [ConfirmFn](/home/lihongyu/projects/geer-agent/src/tools/mod.rs) 保存一个拥有型的授权回调。另一个例子 [TraceContext<'a>](/home/lihongyu/projects/geer-agent/src/agent/mod.rs) 明确借用 Agent 字段，生命周期标注是为短期使用服务的，不应该强改成 static。

## 多个层次的引用怎么读

`&'a &'b str`：外层引用可用到 `'a`，其指向的内部引用受 `'b` 约束。`'a: 'b` 表示 `'a` 至少覆盖 `'b`；`T: 'a` 则约束 T 所含的借用。高阶约束 `for<'a> Fn(&'a str)` 描述“对任意合适的借用期都能调用”，初读闭包生命周期问题时先识别，不必立刻写复杂接口。

练习：函数返回临时 `String` 的引用时，是增加 `'a`，还是返回拥有型 `String`？答案是后者。编译器缺少的不是标签，而是一个能继续持有数据的所有者。

来源：[教程深入生命周期](https://beatai.org/rust-course/advance/lifetime/advance)、[教程 static](https://beatai.org/rust-course/advance/lifetime/static)、[Reference 特征对象生命周期默认值](https://doc.rust-lang.org/reference/lifetime-elision.html#default-trait-object-lifetimes)。
