# 18 全局变量与惰性初始化

[返回总目录](../README.md) · [上一篇](06-threads-and-synchronization.md) · [下一篇](08-unsafe-and-ffi.md)

对应原教程：全局变量，包括常量、静态变量、原子类型、运行期初始化与 Once/Lazy。

## 默认先通过参数传递状态

全局值方便访问，也会隐藏依赖，使测试互相影响。能让 `Config`、`Agent`、`Prompt` 持有的状态，不必全部搬到全局。项目配置就是初始化后传给各组件的。

| 需求 | 常用工具 |
| --- | --- |
| 编译时已知、不需要稳定地址的常量值 | `const` |
| 一个静态存储实例 | `static` |
| 简单共享计数 | `static AtomicU64` 等 |
| 延迟构造一次，固定初始化表达式 | `LazyLock<T>` |
| 稍后显式设置一次或按需初始化 | `OnceLock<T>` |

```rust
use std::sync::LazyLock;

static TOOL_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["read", "write", "bash"]);

fn main() {
    assert!(TOOL_NAMES.contains(&"read"));
}
```

标准库已有这些工具，基础用法不必先添加 `lazy_static`。若已有库承担更复杂的兼容性需求，再按实际需求判断。

## 几个需要改掉的直觉

`static` 不意味着可以任意修改；可变状态仍需锁、原子类型等正确同步。项目禁止新增 unsafe，因此不要用 `static mut` 练习全局配置。

`Box::leak` 会主动放弃正常回收以获得长寿命引用；它可以有特定用途，但不是生命周期报错的通用修复。`'static` 约束也不要求每个拥有型对象永久存活。

项目 [NEXT_RUN_ID](/home/lihongyu/projects/geer-agent/src/agent/mod.rs) 是用途明确的原子计数器；[默认资源限制](/home/lihongyu/projects/geer-agent/src/config/mod.rs) 是 `const ResourceLimits`。二者的区别是一个需要共享的可变实例，另一个提供可复制的默认值。

练习：把配置读取函数设计成接收 `Option<String>` 的纯函数，再比较“修改全局环境后测试”的复杂度。只有真需要唯一实例时，才让全局初始化进入设计。

来源：[教程全局变量](https://beatai.org/rust-course/advance/global-variable)、[OnceLock](https://doc.rust-lang.org/std/sync/struct.OnceLock.html)、[LazyLock](https://doc.rust-lang.org/std/sync/struct.LazyLock.html)。
