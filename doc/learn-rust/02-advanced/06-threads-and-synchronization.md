# 17 线程、消息、锁、原子操作与 Send/Sync

[返回总目录](../README.md) · [上一篇](05-cycles-and-self-reference.md) · [下一篇](07-globals.md)

对应原教程：并发和并行、使用多线程、消息传递、锁/Condvar/信号量、原子操作和内存顺序、Send/Sync。

## 并发不保证并行

并发是多个任务的进度可以交错；并行是多个任务在同一时刻执行。单线程异步可以并发等待多个 I/O；多线程可能利用多核，但也可能争抢同一把锁。

`std::thread::spawn` 创建系统线程，返回 JoinHandle；`join()` 等它结束。普通 spawn 要求闭包适合跨线程并独立存活；`thread::scope` 则允许子线程在受控范围内借用局部数据，作用域结束前会等它们结束。

## 消息传递让所有权跟着消息走

```rust
use std::{sync::mpsc, thread};

fn main() {
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        tx.send(String::from("工具执行完成")).expect("接收端仍在");
    });
    assert_eq!(rx.recv().expect("收到消息"), "工具执行完成");
    worker.join().expect("线程正常结束");
}
```

发送 `String` 后原拥有者不能继续使用该值。阻塞接收会等待消息或所有发送端断开；遍历接收端时若自己还保留一个 sender，循环可能永远等不到“结束”。有界通道还能用容量表达背压。

## 锁、条件变量与信号量解决不同问题

`Mutex<T>` 控制同时只有一个持锁访问者；`RwLock<T>` 允许多读或单写，是否更快要测量。`Condvar` 用于等待某个条件，醒来后要重新检查条件，不能把通知等同于条件必定成立。信号量通过许可证限制并发数量，适合“最多同时读 N 个文件”。

```rust
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let count = Arc::new(Mutex::new(0_u32));
    let shared = Arc::clone(&count);
    let worker = thread::spawn(move || {
        *shared.lock().expect("示例中无人持锁 panic") += 1;
    });
    worker.join().expect("线程结束");
    assert_eq!(*count.lock().expect("锁未中毒"), 1);
}
```

锁守卫离开作用域才解锁。真实代码要决定如何处理锁中毒，不能照抄示例里的 `expect` 到所有业务路径。

## 原子性与内存顺序是两件事

`AtomicU64::fetch_add` 对这个整数执行不可分割的更新；并不表示实现时“暂停所有 CPU”。`Relaxed` 提供该原子对象上的原子性，不建立其他数据的发布/获取顺序。Acquire/Release 等顺序解决跨操作可见性，不能只凭“看起来更快”选择。

[AgentRuntime::new](/home/lihongyu/projects/geer-agent/src/agent/mod.rs) 使用 `NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed)` 取得递增片段；此计数器不用于发布另一块共享数据，所以可以按这个有限用途理解。不要把同样顺序直接照搬到锁、队列或完成标志实现。

## Send 与 Sync 的准确读法

- `T: Send`：T 的所有权可以安全在线程间转移。
- `T: Sync`：`&T` 可以安全在线程间共享，即 `&T: Send`。

通常让编译器根据字段推导，不手写 unsafe 实现。`Rc` 不适合跨线程；`Arc<T>` 是否 Send/Sync 仍依赖 T。某个程序当前只用一个线程，不会让 `tokio::spawn` 的类型约束自动消失。

项目 [本地模拟服务测试](/home/lihongyu/projects/geer-agent/tests/responses_retry.rs) 使用线程接受请求；[Bash 工具](/home/lihongyu/projects/geer-agent/src/tools/bash.rs) 使用 Tokio 任务同时排空 stdout/stderr。这两个例子分别对应系统线程和异步任务。

来源：[教程多线程](https://beatai.org/rust-course/advance/concurrency-with-threads/intro)、[std::thread](https://doc.rust-lang.org/std/thread/index.html)、[Atomic Ordering](https://doc.rust-lang.org/std/sync/atomic/enum.Ordering.html)、[Sync](https://doc.rust-lang.org/std/marker/trait.Sync.html)。
