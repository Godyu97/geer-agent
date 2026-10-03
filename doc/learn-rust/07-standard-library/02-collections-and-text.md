# S2 集合、文本与分配：按访问模式选择

[返回总目录](../README.md) · [上一篇](01-map-and-reading.md) · [下一篇](03-io-path-and-process.md)

## 集合选择由操作决定

| 类型 | 典型访问模式 | 注意点 |
| --- | --- | --- |
| Vec<T> | 顺序扫描、下标、尾部追加 | 扩容与中间插删成本 |
| VecDeque<T> | 队头与队尾操作 | 环形缓冲，不保证全部元素一段连续 |
| HashMap<K,V> | 按键查询 | 迭代次序不作为业务顺序 |
| BTreeMap<K,V> | 有序键、范围查询 | Ord 必须与领域排序匹配 |
| HashSet / BTreeSet | 去重或集合关系 | 稳定输出选择有序类型或排序 |
| BinaryHeap<T> | 优先获取最大项 | 小值优先可用 Reverse 包装 |
| LinkedList<T> | 特定节点链接需求 | 通常先考虑 Vec/VecDeque |

复杂度细节见 [collections 选择与成本表](https://doc.rust-lang.org/std/collections/index.html)。哈希与树的实际性能受键、规模和访问模式影响；不要只背平均复杂度。

## 借用失效与容器增长

```rust,compile_fail
fn main() {
    let mut values = vec![10, 20];
    let first = &values[0];
    values.push(30);
    println!("{first}");
}
```

Vec 增长可能改变元素所在存储，借用检查禁止在仍会使用元素引用时进行这种修改。保留容量不是绕过借用规则的理由。可先复制小值、保存索引，或让借用在修改前结束。[Vec 的容量与保证](https://doc.rust-lang.org/std/vec/struct.Vec.html#guarantees)。

## entry 表达一次逻辑更新

```rust
use std::collections::BTreeMap;

fn main() {
    let mut counts = BTreeMap::new();
    for word in ["rust", "go", "rust"] {
        *counts.entry(word).or_insert(0_u64) += 1;
    }
    assert_eq!(counts.get("rust"), Some(&2));
    assert_eq!(counts.keys().copied().collect::<Vec<_>>(), ["go", "rust"]);
}
```

这里键借用静态字符串，真实输入若来自临时行缓冲并需长期保存，通常应拥有键。面向累计业务时还要考虑 checked_add 和错误策略；示例的小范围输入不会溢出。[BTreeMap::entry](https://doc.rust-lang.org/std/collections/struct.BTreeMap.html#method.entry)。

## UTF-8 的三个长度不能混用

| 量 | Rust 常见接口 | 含义 |
| --- | --- | --- |
| 字节数 | `str::len()`、`as_bytes()` | 传输与缓冲占用 |
| Unicode 标量数 | `chars().count()` | Rust char 的数量 |
| 用户看到的字素簇数 | 通常用 unicode-segmentation | 字符组合、emoji 等显示单元 |

Rust String/str 保证有效 UTF-8；Go string 可以包含任意字节，编码合法性须按用途检查。Rust char 也不是“一个用户可见字符”。[String](https://doc.rust-lang.org/std/string/struct.String.html)、[Go strings/bytes/runes](https://go.dev/blog/strings)、[unicode-segmentation](https://docs.rs/unicode-segmentation/latest/unicode_segmentation/)。

```rust
fn prefix(text: &str, max_chars: usize) -> &str {
    let end = text.char_indices().nth(max_chars)
        .map_or(text.len(), |(index, _)| index);
    &text[..end]
}

fn main() {
    assert_eq!(prefix("Rust学习", 5), "Rust学");
    assert_eq!(prefix("中文", 0), "");
    assert_eq!(prefix("中文", 10), "中文");
    assert_eq!("学".len(), 3);
}
```

此函数按 Unicode 标量截断，不处理字素簇或终端显示宽度。用 char_indices 得到字节边界，避免直接 `&text[..max_chars]` 的 panic。

## 按需拥有：Cow

```rust
use std::borrow::Cow;

fn normalize(input: &str) -> Cow<'_, str> {
    if input.contains('\r') {
        Cow::Owned(input.replace("\r\n", "\n"))
    } else {
        Cow::Borrowed(input)
    }
}

fn main() {
    assert!(matches!(normalize("a\nb"), Cow::Borrowed(_)));
    assert_eq!(normalize("a\r\nb"), "a\nb");
}
```

Cow 让常见路径借用，转换路径拥有；若最终总要保存为 String，优势可能被 `into_owned` 的复制抵消。应按真实使用比例测量。[Cow](https://doc.rust-lang.org/std/borrow/enum.Cow.html)。

## Iterator 管道的成本与短路

`map`、`filter` 等适配器通常是惰性的，`collect`、`sum`、`for_each` 等终结操作才推动消费。`find`、`any` 可以短路，`filter_map` 可以合并筛选与转换，`try_fold` 适合可失败聚合。闭包可能分配或访问 I/O，不能仅凭“迭代器”断定低成本。[Iterator](https://doc.rust-lang.org/std/iter/trait.Iterator.html)。

```rust
fn total(lines: &[&str]) -> Result<u64, std::num::ParseIntError> {
    lines.iter().try_fold(0_u64, |sum, line| {
        Ok(sum + line.parse::<u64>()?)
    })
}

fn main() {
    assert_eq!(total(&["2", "3"]), Ok(5));
    assert!(total(&["2", "bad"]).is_err());
}
```

这段示例只演示解析短路；任意大输入的求和还要增加溢出错误类型。练习：实现一个真正 checked_add 的版本，不以静默饱和掩盖数据错误。

项目对应：检索排序见 [retrieval.rs](../../../src/retrieval.rs)，输出上限见 [bash.rs](../../../src/tools/bash.rs)。解释为什么字节限额、字符截断和 Token 估算需要分别管理。
