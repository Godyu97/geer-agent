# S4 时间、同步与工具类型：理解承诺而非记名字

[返回总目录](../README.md) · [上一篇](03-io-path-and-process.md) · [下一篇：综合练习](05-data-processing-workshop.md)

## 时间分成经过多久与发生何时

| 类型 | 用途 | 不能混用的地方 |
| --- | --- | --- |
| Duration | 一段时长 | 不携带时区或日期 |
| Instant | 截止期限、耗时 | 不是可跨进程持久化的业务时间戳 |
| SystemTime | 记录墙上时间 | 校时可能让比较与差值出现异常 |
| chrono 等 | 日期、时区、格式化 | 另有依赖和业务约束 |

Instant 提供单调时间语义，但跨平台行为、系统休眠与极端范围仍要看文档。不要把一次总预算拆成每轮固定超时后无限延长；为整个操作设 deadline，再计算剩余时间。[Instant](https://doc.rust-lang.org/std/time/struct.Instant.html)、[SystemTime](https://doc.rust-lang.org/std/time/struct.SystemTime.html)。

## 同步类型按状态归属选择

| 类型 | 解决的问题 | 不保证什么 |
| --- | --- | --- |
| Mutex<T> | 保护共享可变状态的独占访问 | 不保证不会死锁或足够低延迟 |
| RwLock<T> | 多读或单写 | 不保证比 Mutex 快、公平策略也依实现 |
| Condvar | 等待状态变化 | 唤醒后必须重新检查条件 |
| Barrier | 多参与者阶段会合 | 一方退出可能使其他参与者卡住 |
| OnceLock / LazyLock | 一次初始化 / 惰性初始化 | 不替代任意后续状态修改 |
| mpsc::channel | 消息传递 | 默认 channel 的排队量可增长 |
| mpsc::sync_channel | 有界同步消息队列 | 发送可能阻塞线程 |
| Atomic* | 特定值的无数据竞争访问 | 不自动建立多个字段的事务 |

PoisonError 是 std 锁的提示机制，不是内存安全修复系统；是否恢复需要重新验证业务不变量。与此相对，Tokio Mutex 不提供同样的 poisoning 机制。[std::sync](https://doc.rust-lang.org/std/sync/index.html)、[Mutex poisoning](https://doc.rust-lang.org/std/sync/struct.Mutex.html#poisoning)、[Tokio Mutex](https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html)。

## 条件变量要围绕状态循环

```text
拿锁
while 条件尚未满足:
    wait：释放锁并等待，返回时重新获得锁
读取或修改满足条件的状态
释放锁
```

通知不是状态本身；在检查和等待之间不能丢失状态变化，虚假唤醒也必须正确处理。关闭标记属于条件的一部分，否则生产者退出后消费者可能永远等待。[Condvar](https://doc.rust-lang.org/std/sync/struct.Condvar.html)。

## 原子操作的两个问题

先问操作自身是否原子，再问它与其他内存读写如何排序。仅统计已完成次数而不借此发布其他数据，Relaxed 往往是合理候选。发布另一个数据结构时，需要证明同步关系，不能只把一个 bool 改成 AtomicBool。

```rust
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

fn main() {
    let completed = Arc::new(AtomicUsize::new(0));
    let worker_count = Arc::clone(&completed);
    let worker = std::thread::spawn(move || {
        worker_count.fetch_add(1, Ordering::Relaxed);
    });
    worker.join().expect("示例线程不会 panic");
    assert_eq!(completed.load(Ordering::Relaxed), 1);
}
```

例子只有计数，并通过 join 等待线程完成，不展示通用的数据发布协议。Acquire/Release、SeqCst 的具体关系见 [atomic 模块](https://doc.rust-lang.org/std/sync/atomic/index.html)。初学阶段复杂共享结构先用锁，避免自行实现无锁算法。

## 标准库工具类型的工程含义

| 类型/方法 | 常见用途 | 设计提醒 |
| --- | --- | --- |
| NonZeroUsize | 类型层限制值不为零 | 构造仍需要检查输入 |
| Reverse<T> | 反转排序 | 比较规则仍来自 T |
| PhantomData<T> | 表达类型或生命周期关系 | 会影响类型约束，不是装饰 |
| Cow<'a,T> | 借用或拥有的统一表示 | 调用 into_owned 可能分配 |
| mem::take / replace | 转交字段，留下有效状态 | 默认值要具有合理语义 |
| Option::take | 消费一次性资源或状态 | None 之后的调用行为要明确 |
| checked_* | 检查整数边界 | 溢出是错误、饱和还是环绕由业务决定 |

精确契约：[NonZero](https://doc.rust-lang.org/std/num/struct.NonZero.html)、[Reverse](https://doc.rust-lang.org/std/cmp/struct.Reverse.html)、[PhantomData](https://doc.rust-lang.org/std/marker/struct.PhantomData.html)。

## 不能靠工具类型解决的业务问题

Arc 不能阻止引用计数循环；Mutex 不能证明锁顺序正确；Result 不能自动进行补偿；AtomicU64 不能替代数据库事务；Instant 不能解决跨主机时钟一致性。把这些限制写进设计，比单纯列依赖更有价值。

项目练习：读 [授权状态](../../../src/ui/app/authorization.rs) 与 [事件缓存](../../../src/ui/app/events.rs)，说明数据为什么在同一锁中改变，什么时候通知其他线程，以及关闭标志如何参与等待条件。
