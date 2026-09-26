# 02 变量、基本类型、表达式与函数

[返回总目录](../README.md) · [上一篇](01-setup.md) · [下一篇](03-ownership.md)

对应原教程：变量绑定与解构；基本类型下的数值、字符/布尔/单元、语句与表达式、函数。

## 变量绑定：名字与值建立关系

```rust
fn main() {
    let input = " 12 ";
    let input = input.trim();
    let turns: u32 = input.parse().expect("示例固定为合法整数");
    let mut remaining = turns;
    remaining -= 1;
    let (name, enabled) = ("read", true);
    assert_eq!((remaining, name, enabled), (11, "read", true));
}
```

`let` 默认不能重新赋值；`mut` 允许修改绑定。第二次 `let input` 是遮蔽，产生新绑定，可改变类型；不是给旧变量开启可变性。元组模式 `(name, enabled)` 一次拆出两个值。

`const MAX: usize = 30;` 是有明确类型的编译期常量。`static` 是有固定存储位置的静态项，进阶篇再讲。`_` 丢弃匹配到的值；`_name` 仍然是绑定，仍可能拿走所有权。

## 常用类型怎么选

| 类型 | 常见用途 | 初学者容易误会的地方 |
| --- | --- | --- |
| `u8` | 一个字节 | 不等于一个中文字符 |
| `u64` / `i64` | 计数、时间戳等 | 有固定范围；有符号与无符号不能随便混算 |
| `usize` | 集合索引和长度 | 宽度随目标平台变化 |
| `f64` | 近似实数 | `0.1 + 0.2` 不保证精确等于 `0.3` |
| `bool` | 条件 | `if` 不接受整数自动转真假 |
| `char` | Unicode 标量值 | 不保证等于用户看到的一个完整字符 |
| `()` | 没有有用返回值 | `Ok(())` 仍明确表达成功 |

算术溢出行为受 overflow-checks 配置影响；不要依赖“debug 会 panic、release 总会绕回”来表达业务规则。需要拒绝溢出用 `checked_add`，封顶用 `saturating_add`，有意环绕用 `wrapping_add`。

```rust
fn main() {
    assert_eq!(u8::MAX.checked_add(1), None);
    assert_eq!(u8::MAX.saturating_add(1), 255);
    assert_eq!(u8::MAX.wrapping_add(1), 0);
    assert_eq!((0..3).collect::<Vec<_>>(), vec![0, 1, 2]);
    assert_eq!((0..=3).count(), 4);
}
```

数值转换不会普遍自动发生。扩大无损转换优先 `u64::from(value)`；可能越界时用 `u8::try_from(value)`。`as` 不会替你验证业务范围。

## 表达式可以产出值

```rust
fn remaining(max: usize, used: usize) -> usize {
    max.saturating_sub(used)
}

fn main() {
    let label = if remaining(3, 1) > 0 { "继续" } else { "停止" };
    assert_eq!(label, "继续");
}
```

函数最后没有分号的表达式就是返回值。把 `max.saturating_sub(used)` 后面加分号，会让函数体结果变成 `()`，和声明的 `usize` 不符。显式 `return` 常用于提前退出；普通结尾不必写。

函数参数需要标注类型，返回类型写在 `->` 后。`-> !` 表示函数不会正常返回，例如永久循环或终止程序；与“正常完成、但没有有用值”的 `()` 不同。

## 对照项目

[AgentRuntime](/home/lihongyu/projects/geer-agent/src/agent/mod.rs) 的 `remaining_turns` 用 `saturating_sub` 计算剩余预算，避免无符号减法下溢；`ResourceLimits` 用 `Option<u64>` 表示可能未配置的上限。[配置解析](/home/lihongyu/projects/geer-agent/src/config/mod.rs) 对浮点值调用 `is_finite()`，因为成功解析成 `f64` 还不代表它适合作为费用或单价。

练习：用函数返回“剩余工具调用数”，分别测试已用数量小于、等于、大于上限。参考实现就是上面的 `remaining`；关键是确定“超过上限返回 0”这一规则。

来源：[教程基本类型](https://beatai.org/rust-course/basic/base-type/index)、[Rust Book 常见编程概念](https://doc.rust-lang.org/book/ch03-00-common-programming-concepts.html)。
