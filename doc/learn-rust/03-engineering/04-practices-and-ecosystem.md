# 28 开发技巧、生态选择与企业案例怎么读

[返回总目录](../README.md) · [上一篇](03-logging-and-tracing.md) · [下一篇](05-linked-lists.md)

对应原教程：企业落地/AWS 案例、常用三方库、命名规范、面试经验、代码开发实践。面试与部分实践条目未形成完整教材，这里提供项目相关补充。

## 企业案例提取方法，不背宣传结论

读 AWS 等案例时，关注它在解决内存、延迟、资源成本还是服务稳定性，以及如何测量。不能把单个场景的改善比例直接套到 Agent 项目；本项目很可能主要等待网络和模型生成，改写一个循环未必改善用户等待时间。

## Rust 命名会提示成本和所有权

| 约定 | 常见含义 | 例子 |
| --- | --- | --- |
| `as_*` | 通常提供轻量借用视图 | `as_str`、`as_ref` |
| `to_*` | 通常产生转换后的新值，可能有成本 | `to_string`、`to_owned` |
| `into_*` | 消费自身并转换 | `into_iter`、`into_bytes` |
| `try_*` | 操作可能失败 | `try_from`、`try_borrow` |
| snake_case | 函数、变量、模块 | `read_input_line` |
| UpperCamelCase | 类型、trait | `TraceRecord`、`ChatProvider` |
| SCREAMING_SNAKE_CASE | 常量/静态项常用风格 | `MAX_RETRIES` |

这类名字是约定，最终成本仍应查看接口和实现。布尔字段尽量使读者能直接理解，例如 `enabled`、`success`；不要同时叠加几个含糊的状态标志。

## 本项目能直接学习的实用写法

1. 只读文本参数用 `&str`，只读序列参数用 `&[T]`，避免无必要地要求调用者持有具体容器。
2. `Path` / `PathBuf` 处理路径，`join` 拼路径，`display()` 负责展示；文件路径不保证是 UTF-8，不要到处强制 `to_str().unwrap()`。
3. 分清 `Instant` 的耗时测量和 `SystemTime` 的墙上时间；Trace 需要时间戳，超时预算更适合单调时钟。
4. 用 `Option::take`、`mem::take` 转交字段内容，保留结构体的有效状态。
5. 为重要失败加上下文，例如是哪种配置值非法；不要在错误消息里顺手打印 API key 或数据库 URL。
6. 让纯解析函数接收值，I/O 边界负责读取环境或文件，测试更稳定。
7. 涉及顺序、重试、幂等时先写出行为契约，再选迭代器或并发工具。

## Serde：类型与 JSON 的桥梁

项目 [TraceRecord](/home/lihongyu/projects/geer-agent/src/trace/mod.rs) 使用 `Serialize` / `Deserialize`；工具参数先作为 `serde_json::Value` 读取，再验证必须字段和类型。

`Value` 适合协议形态不固定的边界，业务形态稳定时具名结构体更容易维护。`Option<T>` 表示字段可缺少/可空的具体行为，还会受 Serde 属性影响；不要只看 Rust 字段类型就推断所有 JSON 兼容规则。

## 依赖选择按问题，而不是按排行榜

| 当前问题 | 已有工具 | 先学的最小内容 |
| --- | --- | --- |
| 异步 I/O | Tokio | runtime、spawn、time、process |
| 流与 Future 组合 | futures-util | StreamExt、join_all |
| JSON 编解码 | serde / serde_json | derive、Value、错误处理 |
| 关系数据库适配 | SeaORM | 查询结果的 Option/Result、实体转换 |
| 文档数据库适配 | mongodb | 文档转换、批量结果边界 |
| 唯一标识与时间 | uuid、chrono、std::time | 标识用途、时区与耗时分开 |

后续真要做 Web，再从候选框架的官方文档核对当前 API 和维护情况；不为一份学习笔记向 Cargo.toml 添加框架。

## 把面试题改成能解释的源码题

“为什么这里用 `&str`？”“`clone` 克隆的是缓冲区还是句柄？”“这个 await 期间谁持有锁？”“Result 外面为什么还有 Option？”这些问题能从具体代码得出可验证答案，比背“Rust 很安全很快”更有帮助。

来源：[教程命名规范](https://beatai.org/rust-course/practice/naming)、[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/naming.html)、[Serde 官方文档](https://serde.rs/)、[std::path](https://doc.rust-lang.org/std/path/index.html)、[std::time](https://doc.rust-lang.org/std/time/index.html)。
