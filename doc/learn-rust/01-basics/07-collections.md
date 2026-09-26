# 07 Vec、HashMap、HashSet 与集合遍历

[返回总目录](../README.md) · [上一篇](06-methods-and-traits.md) · [下一篇](08-lifetimes.md)

对应原教程：集合类型、动态数组 Vector、KV 存储 HashMap；`HashSet`、`BTreeMap` 是项目补充。

## 先按需求选择结构

| 类型 | 适合什么 | 项目对应 |
| --- | --- | --- |
| `Vec<T>` | 有顺序、可增长的一串值 | 会话历史与工具结果 |
| `HashMap<K, V>` | 根据键查值 | Trace 请求 ID 映射 |
| `HashSet<T>` | 去重和存在性判断 | 已授予的工具权限 |
| `BTreeMap<K, V>` | 按键有序地存储和遍历 | 流式工具调用索引 |
| `VecDeque<T>` | 从两端入队出队 | 本项目暂未使用，队列练习可选 |

`HashMap` 的迭代顺序不应作为业务契约。需要恢复调用者顺序时，先建索引，再按输入序列取出。

## `Vec` 的长度、容量与引用

```rust
fn main() {
    let mut results = Vec::with_capacity(4);
    assert_eq!(results.len(), 0);
    results.push(String::from("成功"));
    assert_eq!(results.get(0).map(String::as_str), Some("成功"));
    assert_eq!(results.get(1), None);
    assert_eq!(results.pop().as_deref(), Some("成功"));
}
```

capacity 是已预留空间，len 是实际元素数。直接下标越界会 panic；不确定下标是否合法时用 `get`。增长可能重新分配缓冲区，因此不能一边保留元素引用供后续使用，一边随意 `push`。

## 三种遍历方式决定所有权

```rust
fn main() {
    let mut names = vec![String::from("read"), String::from("write")];
    for name in &names { assert!(!name.is_empty()); }
    for name in &mut names { name.push('!'); }
    let lengths: Vec<usize> = names.into_iter().map(|name| name.len()).collect();
    assert_eq!(lengths, vec![5, 6]);
}
```

`iter()` / `&collection` 给共享引用；`iter_mut()` / `&mut collection` 给独占引用；拥有型集合的 `into_iter()` 交出元素。最后一种之后不能再使用原集合。

## `entry` 一次完成查找和默认插入

```rust
use std::collections::HashMap;

fn main() {
    let mut counts = HashMap::new();
    for name in ["read", "read", "bash"] {
        *counts.entry(name).or_insert(0_u32) += 1;
    }
    assert_eq!(counts.get("read"), Some(&2));
}
```

`entry` 把“已有还是缺少”变成一个可操作的状态；`or_insert` 返回值的可变引用，所以需要 `*` 修改值。生成默认值有成本时用 `or_insert_with` 延迟构造。

[TraceCapture::append_call_delta](/home/lihongyu/projects/geer-agent/src/trace/mod.rs) 使用 `entry(index).or_default()` 找到某个工具调用的累积状态，然后追加本次片段。它使用有序映射，便于按输出索引组织结果。

## 看一个真实的顺序问题

[SqlStore::get_batch](/home/lihongyu/projects/geer-agent/src/dao/sql.rs) 查询多个请求 ID，数据库返回顺序不一定与输入相同。代码将结果放入 `HashMap`，再按 `request_ids.iter()` 重建结果列表。返回类型 `Vec<Option<TraceRecord>>` 还保留了“这个位置不存在”的信息。

练习：输入 ID 为 `["b", "a", "missing"]`，映射只有 a、b。输出必须是 `[Some(b), Some(a), None]`，不能先 `filter_map` 把缺项过滤掉，因为那会破坏位置对应关系。

来源：[教程 Vector](https://beatai.org/rust-course/basic/collections/vector)、[教程 HashMap](https://beatai.org/rust-course/basic/collections/hashmap)、[标准库 collections](https://doc.rust-lang.org/std/collections/index.html)。
