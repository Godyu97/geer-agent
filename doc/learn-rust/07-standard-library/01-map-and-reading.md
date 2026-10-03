# S1 标准库地图与 API 阅读方法

[返回总目录](../README.md) · [下一篇](02-collections-and-text.md)

Go 开发者常习惯从标准库寻找完整网络协议、JSON 和服务工具。Rust 标准库的重点是基础类型、内存与同步、阻塞 I/O、平台接口；JSON、HTTP 客户端、async runtime、随机数和日期格式化等通常通过 crate 补齐。不要把“不在 std”误判为“语言不支持”。[标准库总览](https://doc.rust-lang.org/std/)。

## 从语言、core、alloc 到 std

| 层 | 主要提供什么 | 典型内容 |
| --- | --- | --- |
| 语言 | 编译器理解的语法和约束 | 所有权、引用、enum、trait、async |
| core | 不依赖完整标准运行环境的基础能力 | Option、Result、Iterator、比较、Future |
| alloc | 分配器支持下的拥有容器 | Vec、String、Box、Rc、Arc 等 |
| std | 进一步整合平台能力 | 文件、进程、线程、同步、网络 |
| 生态 crate | 针对领域提供实现与组合 | Tokio、Serde、Axum、Tauri、数据库驱动 |

`#![no_std]` 改变默认库环境，不意味着自动没有堆，也不意味着所有依赖都能用于裸机；能否使用 alloc 取决于分配器与目标环境。[core](https://doc.rust-lang.org/core/)、[alloc](https://doc.rust-lang.org/alloc/)、[Embedded Book](https://doc.rust-lang.org/embedded-book/)。

## 工程问题到模块的地图

| 问题 | 首先查询 | 必须理解的点 |
| --- | --- | --- |
| 文本和字节 | str、String、char、slice | UTF-8、边界、拥有与借用 |
| 结构化数据 | Vec、collections | 排序、增长、哈希与键约束 |
| 转换与格式化 | convert、fmt、str::FromStr | 可失败转换与分配成本 |
| 缺失与失败 | option、result、error | 分支、传播、上下文 |
| 数据转交 | mem、borrow | take/replace/Cow，保留合法状态 |
| 堆与共享 | boxed、rc、sync::Arc、cell | 计数、内部可变性、循环 |
| 文件与路径 | fs、path、ffi | Path 不保证 UTF-8，TOCTOU |
| 流读写 | io、net | 短读短写、EOF、缓冲、上限 |
| 时间预算 | time | Instant 与墙上时间分开 |
| 环境与进程 | env、process | 错误、参数、标准输入和回收 |
| 线程与共享状态 | thread、sync | 作用域、锁、通道、原子顺序 |
| 泛型约束 | marker、ops、cmp、hash | Send/Sync、运算 trait、Eq/Hash |

不按模块列表逐页背诵。先做一项练习，打开相关类型文档，再按下面步骤检查接口。

## 六步读懂一个标准库方法

以 `Vec::retain` 为例：

1. **接收者**：`&mut self` 表示要独占修改容器。
2. **参数**：闭包接收 `&T`，不是转交每个元素。
3. **返回类型**：没有返回新 Vec，修改原对象。
4. **约束**：为什么是 FnMut，是否允许捕获并修改外部计数？
5. **行为保证**：遍历、保留顺序和每项调用次数查看文档。
6. **成本与失败**：是否分配、闭包可能 panic、可否中途退出？

```rust
fn main() {
    let mut visited = 0;
    let mut values = vec![3, 1, 4, 2];
    values.retain(|value| {
        visited += 1;
        value % 2 == 0
    });
    assert_eq!(visited, 4);
    assert_eq!(values, [4, 2]);
}
```

`retain` 的保证见 [Vec::retain](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.retain)。其他同名或类似方法也要重新检查签名，不能只看名称猜测行为。

## 两组常见基础 trait

| 组 | 作用 | 学习重点 |
| --- | --- | --- |
| Debug / Display | 调试表示 / 面向使用者的显示 | Debug 不是稳定协议或自动脱敏 |
| Clone / Copy | 显式克隆 / 允许隐式复制 | 必须看具体类型成本 |
| Default | 建立约定的初始值 | 默认不意味着业务上有效 |
| PartialEq / Eq / PartialOrd / Ord | 比较能力 | 浮点 NaN 与全序、键契约 |
| From / TryFrom / AsRef / Borrow | 转换与借用 | 不同 trait 的承诺不同 |
| Iterator / IntoIterator / FromIterator | 遍历与聚合 | 惰性、消费和目标类型 |
| Read / Write / BufRead / Seek | 阻塞字节流能力 | 成功可能只处理部分数据 |

prelude 只自动引入常见项，并不会导入整个 std。遇到陌生方法，可以查看该类型的 inherent methods 与 trait implementations，确认方法究竟来自哪个 trait。[prelude](https://doc.rust-lang.org/std/prelude/index.html)。

## 项目阅读任务

学完后继续 [借用解析与拥有汇总综合练习](05-data-processing-workshop.md)，将签名阅读变成可运行的程序。

从 [配置路径](../../../src/config/path.rs)、[文件工具](../../../src/tools/file.rs)、[工具输出限制](../../../src/tools/bash.rs) 各挑五个 std API。填写：签名、所有权、错误或 panic 条件、资源成本、项目调用理由。完成这张表，比背诵上百个函数更能提升阅读能力。
