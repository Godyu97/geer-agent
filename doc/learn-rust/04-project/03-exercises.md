# 34 分阶段练习与参考答案

[返回总目录](../README.md) · [上一篇](02-reading-a-request.md)

练习优先放独立 scratch 工程，不修改项目业务。下面的标准库代码块可独立运行；参考答案紧跟题目，建议先遮住答案自己写。

## 练习一：解释一个函数签名

题目：`fn parse_input(line: &str) -> Input` 为什么不接收 String？返回 Input 含有 String 时，是否能直接保存 line？

答案：函数只读输入，借用 `&str` 适用于更多调用者。返回的消息如果要独立拥有内容，需要 `to_owned()` 等明确复制；如果改为保存引用，整个 Input 类型与调用者就要承担相应生命周期约束。项目选择拥有型枚举，调用更直观。

## 练习二：避免移动后的再次使用

题目：写一个只计算工具名长度的函数，调用后原字符串还能使用。

```rust
fn tool_name_len(name: &str) -> usize { name.len() }

fn main() {
    let name = String::from("read");
    assert_eq!(tool_name_len(&name), 4);
    assert_eq!(name, "read");
}
```

验收：不通过 clone 解决，不取得 String 所有权。

## 练习三：可选配置，但非法值必须报错

题目：`None` 表示不设置上限，字符串必须能解析为正整数。

```rust
fn limit(value: Option<&str>) -> Result<Option<u64>, String> {
    value.map(|raw| {
        let n = raw.trim().parse::<u64>().map_err(|_| "不是整数".to_owned())?;
        if n == 0 { Err("必须大于零".to_owned()) } else { Ok(n) }
    }).transpose()
}

fn main() {
    assert_eq!(limit(None), Ok(None));
    assert_eq!(limit(Some(" 3 ")), Ok(Some(3)));
    assert!(limit(Some("0")).is_err());
    assert!(limit(Some("bad")).is_err());
}
```

对照 [配置解析](/home/lihongyu/projects/geer-agent/src/config/mod.rs)；解释 transpose 前后的完整类型。

## 练习四：不破坏 UTF-8 的前缀

题目：取前两个 Unicode 标量值，不能直接用字节 `[..2]`。

```rust
fn prefix(text: &str, n: usize) -> String { text.chars().take(n).collect() }

fn main() {
    assert_eq!(prefix("你好Rust", 2), "你好");
    assert_eq!(prefix("a", 2), "a");
    assert_eq!(prefix("", 2), "");
}
```

边界：这是标量值前缀，不是所有 Unicode 字素簇的截断算法。对照 [Bash 输出截断](/home/lihongyu/projects/geer-agent/src/tools/bash.rs)。

## 练习五：按输入顺序返回查询结果

题目：数据表中有 a、b，查询 b、missing、a，保留缺项。

```rust
use std::collections::HashMap;

fn main() {
    let stored = HashMap::from([("a", 1), ("b", 2)]);
    let ids = ["b", "missing", "a"];
    let result: Vec<_> = ids.iter().map(|id| stored.get(id).copied()).collect();
    assert_eq!(result, vec![Some(2), None, Some(1)]);
}
```

验收：不能直接遍历 HashMap，也不能 filter_map 丢失位置。对照 [DAO 批量查询](/home/lihongyu/projects/geer-agent/src/dao/sql.rs)。

## 练习六：把 pending 一次移动进 history

```rust
fn commit(history: &mut Vec<String>, pending: &mut Vec<String>) {
    history.extend(std::mem::take(pending));
}

fn main() {
    let mut history = vec!["旧消息".to_owned()];
    let mut pending = vec!["本轮输入".to_owned(), "本轮回答".to_owned()];
    commit(&mut history, &mut pending);
    assert_eq!(history.len(), 3);
    assert!(pending.is_empty());
    commit(&mut history, &mut pending);
    assert_eq!(history.len(), 3);
}
```

验收：不克隆每个 String，提交后 pending 为空，再提交不会重复追加。对照 [Prompt::commit_turn](/home/lihongyu/projects/geer-agent/src/prompt/conversation.rs)。

## 练习七：辨别并发与取消

题目：对 `JoinHandle` 的等待超时，后台任务一定停了吗？`join_all` 会按完成先后返回吗？

答案：超时取消哪一层要看传入的 Future，丢弃 JoinHandle 通常让任务继续；需要明确中止与等待策略，阻塞任务另有局限。`join_all` 按输入顺序给结果，不是完成顺序。阅读 [工具批次](/home/lihongyu/projects/geer-agent/src/tools/mod.rs) 并画出 read/read/write/read 的执行阶段。

## 练习八：选择验证层次

题目：新增一个本地命令，应该分别验证哪些行为？

答案：解析函数检查该命令、空白和未知命令；交互层检查可见输出和状态改变；若不需要模型，确认没有误发请求。只改本地命令不必调用真实模型证明语法正确。

可阅读并自行运行现有定向测试：

```bash
cargo test parses_supported_commands
cargo test --test responses_retry
```

这些命令会编译当前项目；本次笔记验证没有把它们当成已执行结果。外部数据库和真实上游验证也需单独说明。

## 自评

能解释参数的拥有/借用、看到 Result 就想到失败路径、能区分同步阻塞与异步等待、知道怎样验证输出和状态，就可以开始给项目做小的能力切片。遇到难点回查对应篇章，不需要先掌握 unsafe 双向链表。
