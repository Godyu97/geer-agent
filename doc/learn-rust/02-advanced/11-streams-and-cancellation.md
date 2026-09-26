# 22 Stream、并发组合、超时与取消

[返回总目录](../README.md) · [上一篇](10-async-future-and-pin.md) · [下一篇](12-web-server.md)

对应原教程：async/await 与 Stream、同时运行多个 Future；同时补充项目中的重试和副作用边界。

## Iterator、Future 和 Stream 的区别

| 抽象 | 获取结果的方式 | 结果数量 |
| --- | --- | --- |
| `Iterator` | 同步 `next()` | 零到多个 |
| `Future` | 轮询，通常通过 `.await` | 一个最终输出 |
| `Stream` | 异步逐项取得结果 | 零到多个 |

项目通过 `futures_util::StreamExt` 扩展流，`stream.next().await` 返回 `Option<Item>`。若 Item 自己也是 Result，就需要分别处理“流结束”和“本条失败”。

## 并发组合并不只有 spawn

| 工具 | 等待规则 | 容易忽视的边界 |
| --- | --- | --- |
| 先 `a.await` 再 `b.await` | 顺序推进 | 第二个可能到后面才开始执行 |
| `join!` / `join_all` | 并发推进，等全部结束 | 同一任务内的并发不保证多核并行 |
| `try_join!` | 可在错误时提前返回 | 未完成分支的取消和副作用要考虑 |
| `tokio::select!` | 选择就绪分支 | 其他传入 Future 可能被丢弃 |
| `tokio::spawn` | 独立调度一个任务 | 返回 JoinHandle；丢弃句柄通常不会停止任务 |

教程某些章节使用 futures 的 `select!`，它与 Tokio 的同名宏规则不完全相同，不把 `FusedFuture` 等要求直接混用。

## 项目如何保持读取并发和写入顺序

[Tools::execute_batch](/home/lihongyu/projects/geer-agent/src/tools/mod.rs) 将连续的 read 组成一组，提前完成准备与授权，然后使用 `spawn_blocking` 执行同步读，`join_all` 收集结果。`join_all` 保留输入顺序，不是完成得早就排前面。遇到 write/edit/bash 等顺序操作时，先处理完前一组。

```mermaid
flowchart LR
    A[read A 与 read B] --> B[并发读取，按输入顺序汇总]
    B --> C[write C]
    C --> D[下一组 read D 与 read E]
    D --> E[并发读取，按输入顺序汇总]
```

并发许可是在业务层做出的决定，不是因为“都是 async 函数就可以全部一起跑”。

## 超时丢弃 Future，不自动撤销现实世界

`tokio::time::timeout` 到期返回错误，并取消它直接管理的 Future；若 Future 长时间不让出执行机会，超时也不能强行抢占同步运算。已经发送的 HTTP 请求、已经写入的文件不会自动回滚。

对 JoinHandle 超时或丢弃它，背后的任务还可能继续；已开始的 `spawn_blocking` 工作通常不能通过 abort 中止。要追踪“被取消的是哪一层”。本项目 [Bash 工具](/home/lihongyu/projects/geer-agent/src/tools/bash.rs) 显式处理子进程超时与读取任务，不能把一般 timeout 当作进程管理器。

## 重试为什么要看是否已经显示正文

[Responses::request_reply](/home/lihongyu/projects/geer-agent/src/provider/openai/responses.rs) 设置最多 5 次额外重试，即最多 6 次请求尝试；每次重试前显示进度。一旦正文已交给终端，当前逻辑停止自动重试，避免把一段回答重复显示。

流的 EOF 不代表业务成功；`collect_reply` 等到明确完成事件才认可完成。项目还区分流的空闲超时和整个 Agent 请求预算期限。读取文字片段、记录会话、执行工具是三个不同阶段，不能把“打印过几个字”当成“本轮已经成功提交”。

练习：若模型输出一半断线，为什么简单重试可能有副作用？至少考虑重复文字、重复工具动作和已保存历史。进一步阅读 [请求流程](../04-project/02-reading-a-request.md)。

来源：[教程多个 Future](https://beatai.org/rust-course/advance/async/multi-futures-simultaneous)、[Tokio select](https://tokio.rs/tokio/tutorial/select)、[timeout](https://docs.rs/tokio/latest/tokio/time/fn.timeout.html)、[spawn_blocking](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)。
