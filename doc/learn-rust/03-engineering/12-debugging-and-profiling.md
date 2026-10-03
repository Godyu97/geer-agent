# 调试与性能诊断：从现象找到成本

[返回总目录](../README.md) · [上一篇](11-production-services.md) · [下一篇](13-workspace-ci-and-release.md)

## 先区分五种慢

| 现象 | 需要的证据 | 常见下一步 |
| --- | --- | --- |
| 编译慢 | Cargo timings、依赖与 feature | 减少未用依赖、泛型实例或 native 构建 |
| CPU 忙 | CPU profiler、热点调用栈 | 算法与重复计算优先 |
| 内存高 | 峰值、分配速率、存活数据 | 区分队列积压、缓存与泄漏 |
| CPU 不忙但延迟高 | span、队列、锁、外部等待 | 连接池、阻塞、背压和上游 |
| 少数请求极慢 | 延迟分布与关联上下文 | 尾部输入、资源饱和、重试 |

工程师必须能把一个测量结果与一种解释连接起来，不能只说“Rust 零成本，所以应该很快”。泛型和迭代器给优化提供机会，分配、哈希、引用计数、锁与缓存访问仍是真实成本。[Rust Performance Book](https://nnethercote.github.io/perf-book/)。

## 编译错误与运行问题用不同工具

编译时先找首个相关错误和 E 编号，画出拥有者、借用范围及跨 await 的值；查 rustc --explain。运行时 panic 用 backtrace 与调试符号；程序无 panic 但结果错，要靠业务状态、测试与日志判断。

```bash
rustc --explain E0382
cargo build --timings
```

独立练习需要运行并观察 backtrace 时，可通过：

```bash
scripts/test-safe.sh env RUST_BACKTRACE=1 cargo run --manifest-path <练习包>/Cargo.toml
```

`<练习包>` 是要替换的占位符。安全入口不会自动转发所有自定义环境变量，因此用 env 显式设置必要、非敏感的参数。不要输出真实模型配置来调试。

## 生成便于诊断的 release 程序

在独立练习包中可以保留 release 的行号/符号信息：

```toml
[profile.release]
debug = 1
```

实际使用 GDB/LLDB、rust-gdb/rust-lldb 或系统 profiler 时，工具、权限、优化后的变量可见性和符号文件都要核对。不要为了 profiling 默认打开全部 debug、不加限制启动长期服务，或修改宿主系统全局策略。[profile debug](https://doc.rust-lang.org/cargo/reference/profiles.html#debug)。

## Rust 中容易忽略的成本

| 写法 | 可能成本 | 如何验证 |
| --- | --- | --- |
| 循环中 String clone | 每轮分配与复制 | 分配 profile、输入规模曲线 |
| Arc clone/drop 高频 | 原子计数与共享缓存线 | 共享热点与并发缩放 |
| Vec 反复增长 | 分配与搬迁 | 容量、峰值、reserve 对照 |
| HashMap 迭代 | 不只由 len 决定 | 实际容量与遍历数据量 |
| 过大 enum/Future | 更多状态和缓存占用 | size_of、任务数、实际存活范围 |
| await 前保存大缓冲 | 等待期间保留内存 | 生命周期与高并发峰值 |
| dyn/Box | 分派与可能的分配 | 热点与冷路径分别测量 |
| 日志格式化/过量字段 | CPU、分配和输出 I/O | 过滤前后、采样策略 |

这些是检查线索，不是必须消除的写法。清楚的独立所有权和可维护接口可能比少一次 clone 更值得保留。

## 基准需要输入分布和对照

Criterion 适合统计基准，标准库 black_box 帮助避免部分无用计算被优化掉，不能保证基准完全反映真实场景。release/profile、工具链、硬件、输入、并发、预热与运行次数都写进记录。[Criterion](https://bheisler.github.io/criterion.rs/book/)、[black_box](https://doc.rust-lang.org/std/hint/fn.black_box.html)。

本仓库执行 benchmark 同样用受限入口，例如 `scripts/test-safe.sh cargo bench --manifest-path <练习包>/Cargo.toml`。限制样本量和总运行时间，避免多个完整 benchmark 同时竞争资源。

练习：比较同一文本的顺序 char_indices 遍历与反复 chars().nth(i)。输入规模逐步增加至明确上限，解释趋势来自 O(n) 与反复扫描，而不是先归因于“迭代器更快”。

## 并行不一定提速

Rayon 将迭代/任务分给 CPU 线程池，适合足够大的独立 CPU 工作。拆分、调度、合并、内存带宽和锁会形成成本；Tokio 的 async I/O 等待与 Rayon CPU 并行是不同问题。[Rayon](https://docs.rs/rayon/latest/rayon/)。

不要在 Tokio worker 中直接执行长 Rayon 阻塞等待；设计明确的受限桥接和 CPU 资源预算。任务太小或依赖串行写入时，并行反而可能更慢。

## 辅助验证工具各证明什么

| 工具 | 能发现什么 | 不能当成什么 |
| --- | --- | --- |
| cargo-llvm-cov | 源码覆盖情况 | 业务正确性比例 |
| proptest | 生成输入检查性质 | 所有可能输入证明 |
| cargo-fuzz | 在预算内寻找崩溃/异常 | 无漏洞证明 |
| Miri | 支持范围内的未定义行为等 | 全平台、全路径真实执行替代 |
| Loom | 在受控模型下探索同步交错 | 自动理解任意未改造并发程序 |

来源：[coverage](https://github.com/taiki-e/cargo-llvm-cov)、[proptest](https://proptest-rs.github.io/proptest/proptest/index.html)、[fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html)、[Miri](https://github.com/rust-lang/miri)、[Loom](https://docs.rs/loom/latest/loom/)。这些是进阶工具，本项目没有因为写教程就引入它们；运行同样需要资源上限与适配环境。

## 一份合格的优化记录

```text
问题：什么场景、什么输入、用户受到什么影响
基线：版本、profile、平台、样本、延迟与内存
证据：哪段代码、哪种成本、如何排除其他解释
改动：只改变一个因素，保留业务行为
结果：同条件测量，报告收益和波动
限制：适用输入、额外内存、复杂度与回退路径
```

项目任务：选检索分块、工具输出截断或请求历史构造中的一项，先说明它是否在当前用户延迟的主要路径，再决定是否值得优化。
