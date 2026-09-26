# 14 类型转换、newtype、别名与 DST

[返回总目录](../README.md) · [上一篇](02-closures-and-iterators.md) · [下一篇](04-smart-pointers.md)

对应原教程：深入类型中的类型转换、newtype 和类型别名、Sized/DST、枚举和整数。

## 转换方法表达不同承诺

| 写法 | 适用场景 |
| --- | --- |
| `Target::from(value)` / `value.into()` | 约定为不失败的值转换 |
| `Target::try_from(value)` / `try_into()` | 转换可能失败 |
| `text.parse::<T>()` | 由 `FromStr` 解析文本 |
| `value as T` | Rust 定义的强制转换，数值转换可能截断 |
| `as_ref` / `as_deref` | 获得引用视图，通常不是复制数据 |

```rust
fn main() {
    assert_eq!(u64::from(12_u32), 12);
    assert!(u8::try_from(300_u16).is_err());
    assert_eq!(300_u16 as u8, 44);
    let owned = Some(String::from("read"));
    let view: Option<&str> = owned.as_deref();
    assert_eq!(view, Some("read"));
    assert!(owned.is_some());
}
```

项目把 SDK 的 Token 计数转成 `u64` 使用 `u64::from`；记录重试次数时使用 `i32::try_from(...).unwrap_or(i32::MAX)` 表达可见的上限策略。

## newtype 会创建新类型，别名不会

```rust
#[derive(Debug, PartialEq, Eq)]
struct RequestId(String);
type Label = String;

fn main() {
    let id = RequestId(String::from("req-1"));
    let label: Label = String::from("read");
    assert_eq!(id.0, "req-1");
    assert_eq!(label, "read");
}
```

`RequestId` 可防止把模型名当成请求 ID 传入，也可承载自己的 trait 实现。`Label` 只是更好读的名字，仍完全等价于 `String`。[TraceError(pub(crate) String)](/home/lihongyu/projects/geer-agent/src/trace/mod.rs) 是项目现有的 newtype；`ConfirmFn` 是给复杂闭包类型起别名。

## 动态大小类型必须隔着合适的指针使用

`str`、`[T]`、`dyn Trait` 的大小通常不能在编译时作为独立值确定；`&str`、`&[T]`、`Box<dyn Trait>` 则有可确定的指针表示。泛型默认要求 `Sized`，`T: ?Sized` 是放宽此要求，通常同时接收 `&T` 等间接访问方式。

```rust
fn bytes<T: AsRef<[u8]> + ?Sized>(value: &T) -> usize {
    value.as_ref().len()
}

fn main() {
    assert_eq!(bytes("read"), 4);
}
```

这里允许 `T = str`，但参数仍是 `&T`。它不是说可以把任意大小值直接塞到一个固定大小栈槽里。

## 枚举与数字之间不要自行假设布局

无字段枚举可以有显式判别值，某些情况下可以 `as` 转成整数；整数转回枚举需要检查合法值，通常实现 `TryFrom` 或 `match`。带数据的枚举还包含有效载荷，不能按“一个整数”来理解。

项目的 `TraceStatus` 通过 Serde 命名规则序列化为字符串，这与 Rust 内存中的枚举判别值没有绑定关系。对外协议用明确序列化规则，不依赖编译器默认布局。

来源：[教程深入类型](https://beatai.org/rust-course/advance/into-types/intro)、[标准库转换](https://doc.rust-lang.org/std/convert/index.html)、[Reference 动态大小类型](https://doc.rust-lang.org/reference/dynamically-sized-types.html)。
