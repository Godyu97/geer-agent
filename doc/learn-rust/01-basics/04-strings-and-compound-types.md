# 04 字符串、切片、元组、结构体、枚举与数组

[返回总目录](../README.md) · [上一篇](03-ownership.md) · [下一篇](05-control-flow-and-patterns.md)

对应原教程：“复合类型”下全部小节；也可回查“Rust 难点攻关”的切片、字符串条目。

## `String`、`str` 和 `&str`

`String` 拥有可增长的 UTF-8 字节缓冲区。`str` 是动态大小的文本类型，通常隔着引用使用；`&str` 是借来的文本视图，携带位置与字节长度。字符串字面量的类型通常是 `&'static str`。

| 需求 | 常用选择 |
| --- | --- |
| 只读文本参数 | `&str` |
| 保存独立文本、追加内容 | `String` |
| 只读若干同类型元素 | `&[T]` |
| 修改已有元素但不改变长度 | `&mut [T]` |
| 元素数量固定在类型中 | `[T; N]` |
| 元素数量动态变化 | `Vec<T>` |

```rust
fn main() {
    let mut text = String::from("读Rust");
    assert_eq!(text.len(), 7);
    assert_eq!(text.chars().count(), 5);
    assert_eq!(text.get(..3), Some("读"));
    assert_eq!(text.get(..1), None);
    text.push('!');
    let first_two: String = text.chars().take(2).collect();
    assert_eq!(first_two, "读R");
}
```

字符串的下标范围使用**字节位置**；边界落在 UTF-8 字符内部时，直接切片会 panic，`get` 会返回 `None`。`chars()` 遍历 Unicode 标量值，组合音标、部分 emoji 仍可能由多个标量值组成；不要把它直接叫“屏幕字符计数”。

`trim()` 返回借用视图，`to_owned()` 生成拥有型文本；`push_str` 追加文本，`format!` 生成新 `String`。字符串拼接运算 `String + &str` 会消耗左边的 `String`，不等同于两个字符串都只读。

## 元组：少量有关联、类型可不同的值

```rust
fn main() {
    let result = (String::from("输出内容"), true);
    assert!(result.1);
    let (text, success) = result;
    assert_eq!(text, "输出内容");
    assert!(success);
}
```

本项目 [Bash 工具](/home/lihongyu/projects/geer-agent/src/tools/bash.rs) 返回 `(String, bool)`，依次表达文本与成功状态。字段更多或容易搞混时，用命名结构体通常更易读。

## 结构体表达“同时拥有这些字段”

```rust
#[derive(Debug)]
struct ToolCall {
    name: String,
    args: String,
}

fn main() {
    let name = String::from("read");
    let call = ToolCall { name, args: String::from("{}") };
    assert_eq!(call.name, "read");
    assert_eq!(call.args, "{}");
}
```

`ToolCall { name, ... }` 是字段简写；`..other` 是结构体更新语法，可能移动 `other` 的非 `Copy` 字段。不要以为它自动克隆原对象。[provider::ToolCall](/home/lihongyu/projects/geer-agent/src/provider/mod.rs) 的实际版本还包括调用 ID。

## 枚举表达“处于其中一种形态”

```rust
enum Input {
    Exit,
    Message(String),
}

fn main() {
    let input = Input::Message(String::from("你好"));
    let text = match input {
        Input::Exit => String::from("bye"),
        Input::Message(text) => text,
    };
    assert_eq!(text, "你好");
    let _ = Input::Exit;
}
```

枚举变体可以带不同字段，因此比“几个 bool 组合状态”更容易保证合法性。`Option<T>` 就是 `Some(T)` 或 `None`；`Result<T, E>` 是 `Ok(T)` 或 `Err(E)`。

[repl::Input](/home/lihongyu/projects/geer-agent/src/repl/index.rs) 区分退出、帮助、消息等输入；[Prompt::State](/home/lihongyu/projects/geer-agent/src/prompt/conversation.rs) 区分两种协议的历史结构。它们比字符串标记更方便让编译器检查分支遗漏。

## 数组与切片的关系

```rust
fn sum(values: &[u64]) -> u64 {
    values.iter().sum()
}

fn main() {
    let array = [2, 3, 5];
    let vector = vec![2, 3, 5];
    assert_eq!(sum(&array), 10);
    assert_eq!(sum(&vector[..2]), 5);
}
```

`[T; 3]` 和 `[T; 4]` 是不同类型；`&[T]` 可以借用它们的一段。切片的“不定长”指长度不编码在类型大小中，不代表某个已创建的切片引用会自动增长。

项目读取输入时先用字节缓冲，再尝试 `String::from_utf8`；Bash 输出展示则用 `from_utf8_lossy`。前者拒绝非法文本，后者替换非法序列，选择反映的是产品处理策略。

练习：预测 `"你好".len()`、`"你好".chars().count()` 和 `"你好".get(..1)`。答案分别为 `6`、`2`、`None`。

来源：[教程复合类型](https://beatai.org/rust-course/basic/compound-type/intro)、[标准库 String](https://doc.rust-lang.org/std/string/struct.String.html)、[标准库 str](https://doc.rust-lang.org/std/primitive.str.html)。
