# 术语地图与阶段自测

[返回总目录](../README.md) · [学习路线](../00-engineer-roadmap.md)

## 术语先对应工程问题

| 术语 | 含义与阅读线索 |
| --- | --- |
| ownership | 谁负责值的生命周期与转交 |
| move | 转移所有权，原位置通常不能再使用 |
| borrow | 在约束范围内访问另一个拥有者的数据 |
| lifetime | 表达引用有效性关系，不是给对象续命 |
| NLL | 借用范围按使用分析，不总等于整个词法块 |
| RAII / Drop | 资源跟随拥有值与作用域释放，异常终止有边界 |
| interior mutability | 通过特定类型在共享引用下管理变化 |
| Send | 值可以安全转移到另一线程的约束 |
| Sync | 共享引用可以安全跨线程的约束 |
| static dispatch | 通过具体类型/泛型确定调用实现 |
| dyn compatibility | trait 能否经特征对象使用的规则 |
| monomorphization | 对具体泛型实例生成代码 |
| coherence / orphan | trait 实现的一致性与外部实现限制 |
| newtype | 用包装建立新的类型身份与约束 |
| typestate | 不同类型阶段限制可调用的方法 |
| HRTB / GAT | 对任意生命周期成立的 bound / 带参数的关联类型 |
| Future / poll | 可推进的异步计算及推进契约 |
| Waker | 请求执行器以后再次轮询任务 |
| executor / driver | 调度计算 / 处理 I/O 与计时唤醒来源 |
| Pin / Unpin | 地址相关约束 / 可不受 pin 限制的类型能力 |
| task / thread | executor 调度单位 / OS 执行线程 |
| cancellation safety | 丢弃并重建等待是否丢失所需进度 |
| backpressure | 消费能力不足向生产路径传播等待/拒绝 |
| framing | 字节流到业务消息的边界解析 |
| idempotency | 同一逻辑操作重复提交仍符合定义的结果 |
| deadline | 整个操作共享的最终期限 |
| extractor / IntoResponse | 请求到类型 / 返回值到 HTTP 响应 |
| Service / Layer | 请求处理与 readiness / 对服务的包装组合 |
| IPC / capability | 跨进程消息边界 / 窗口可用能力配置 |
| edition / MSRV | 包语言规则版本 / 最低支持编译器政策 |
| feature / profile | 编译能力开关 / 构建优化与调试策略 |
| crate / package / workspace | 编译单位 / Cargo 包 / 多包协作工程 |

精确规则按正文主题查 [Reference](https://doc.rust-lang.org/reference/) 和 [std](https://doc.rust-lang.org/std/)，这些简释不能替代类型签名与库契约。

## 三组阶段自测

| 阶段 | 必须能解释或实现的任务 | 对应章节 |
| --- | --- | --- |
| 基础稳定 | 在 Vec 修改前结束元素借用，区分 String move 和 Arc clone | [所有权 API](../06-go-to-rust/02-ownership-and-api.md) |
| 基础稳定 | 返回局部处理结果时选择 String 或合法借用 | [生命周期](../01-basics/08-lifetimes.md) |
| 基础稳定 | 区分找不到、操作失败和非法输入 | [类型错误](../06-go-to-rust/03-types-traits-and-errors.md) |
| 工程入门 | 写有限字节读取和 UTF-8 安全预览 | [标准库](../07-standard-library/03-io-path-and-process.md) |
| 工程入门 | 一个任务唯一拥有状态，生产者有背压 | [Tokio 实战](../08-ecosystem/04-tokio-workshop.md) |
| 工程入门 | 解释超时、任务句柄丢弃和提交结果未知 | [Tokio 原理](../08-ecosystem/03-tokio-runtime.md) |
| 独立交付 | 画出 HTTP 或 IPC 请求的全部边界 | [Axum](../08-ecosystem/05-axum-and-tower.md)、[Tauri](../08-ecosystem/08-tauri-architecture.md) |
| 独立交付 | 在相关失败路径准确保存状态和退出 | [服务工程](../03-engineering/11-production-services.md) |
| 独立交付 | 用基线定位瓶颈，说明平台和版本 | [诊断](../03-engineering/12-debugging-and-profiling.md) |
| 独立交付 | 另一工程师能运行、审查并复现 | [毕业项目](../04-project/04-capstone-and-rubric.md) |

## 最容易保留下来的错误直觉

| 错误直觉 | 修正后应采用的检查 |
| --- | --- |
| 生命周期注解能延长对象寿命 | 找真实拥有者与释放点 |
| Arc 包装任何东西都能跨线程 | 看内部 T 和 Send/Sync 条件 |
| async 调用自动在后台执行 | 找 poll/await/spawn 和 runtime |
| 每个 await 都会让出执行 | 检查 Future 是否立即 Ready 和循环预算 |
| timeout 可以打断任意代码 | 找可调度点、句柄和外部副作用 |
| 有界 channel 保证整体内存有界 | 数量、单项字节、在途工作分别限制 |
| 编译通过意味着业务和并发都正确 | 测状态、死锁、取消与提交路径 |
| release 总能复现任何基准 | 固定输入、环境、profile 与测量方法 |
| 所有 trait 都能变成 dyn | 查 dyn compatibility 和关联类型 |
| 学过很多 crate 就是合格 | 独立完成一项可验收交付 |

完成自测后继续做真实小任务；遇到新问题时回到签名、状态和资源，而不是不断增加依赖。
