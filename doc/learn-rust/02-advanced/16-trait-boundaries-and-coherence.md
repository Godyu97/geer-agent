# Trait 深入：一致性、动态边界与异步返回约束

[返回总目录](../README.md) · [Go 类型对照](../06-go-to-rust/03-types-traits-and-errors.md) · [类型系统进阶](14-type-system-and-layout.md)

Go 的隐式接口经验能帮助理解替换边界；Rust 还需要明确实现归属、是否允许重叠、对象能调用哪些方法，以及 Future 的 Send/lifetime 保证。

## orphan rule 为什么影响扩展设计

不能在自己的 crate 任意为外部类型实现外部 trait。下面两个定义都来自标准库，会被拒绝：

```rust,compile_fail
use std::fmt;

impl fmt::Display for Vec<u8> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} bytes", self.len())
    }
}

fn main() {}
```

用本地 newtype 把语义和实现归属放在自己的边界中：

```rust
use std::fmt;

struct Packet(Vec<u8>);

impl fmt::Display for Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} bytes", self.0.len())
    }
}

fn main() {
    assert_eq!(Packet(vec![1, 2]).to_string(), "2 bytes");
}
```

完整规则还涉及 trait 参数中的本地类型、未覆盖参数和 fundamental 类型，不能概括成“只要代码写在本地模块就行”。它帮助避免多个依赖各自提供同一实现导致选择冲突。[coherence/orphan rules](https://doc.rust-lang.org/reference/items/implementations.html#trait-implementation-coherence)。

## blanket impl 会占用一整片实现空间

```rust,compile_fail
trait Label {}
impl<T> Label for T {}

struct Item;
impl Label for Item {}

fn main() {}
```

第一个实现已覆盖 Item，第二个发生重叠。公共库新增 blanket impl 可能影响下游已有实现与推断，审查时应同时检查兼容性。不要为规避冲突随意把 trait、类型和所有依赖重写。[overlapping implementations](https://doc.rust-lang.org/reference/items/implementations.html#trait-implementation-coherence)、[SemVer](https://doc.rust-lang.org/cargo/reference/semver.html)。

## dyn compatibility 是逐项规则

trait 有一个无法在对象上分派的泛型方法时，可用 `where Self: Sized` 把该方法限定为具体类型调用，让其余方法保留动态接口：

```rust
trait Sink {
    fn write(&mut self, text: &str);
    fn write_value<T: ToString>(&mut self, value: T) where Self: Sized {
        self.write(&value.to_string());
    }
}

impl Sink for String {
    fn write(&mut self, text: &str) { self.push_str(text); }
}

fn main() {
    let mut buffer = String::new();
    buffer.write_value(42);
    let dynamic: &mut dyn Sink = &mut buffer;
    dynamic.write("!");
    assert_eq!(buffer, "42!");
}
```

dynamic 不能调用 write_value，它不是该对象提供的分派能力。associated types、返回 Self、不透明返回与 async 方法也有相应规则，不靠给类型加 Box 自动修复。[dyn compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility)。

## async trait：Send 需要成为明确契约

原生 async fn in trait 可以用于静态分派，但 async 本身不承诺其返回 Future 满足 Send。公共接口若需要把调用放进可迁移任务，必须明确返回 Future 的约束；不能只给 trait 加 Send 就以为所有方法的 Future 都 Send。

下面展示静态接口与显式装箱动态接口。所有 Future 在本例第一次 poll 即完成；这不是通用 block_on 或 I/O 执行器。

```rust
use std::{future::{Future, ready}, pin::{pin, Pin}, task::{Context, Poll, Waker}};

trait StaticFetch {
    fn fetch(&self, key: u64) -> impl Future<Output = Option<String>> + Send;
}

trait DynamicFetch: Sync {
    fn fetch(&self, key: u64) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>>;
}

struct Memory { value: String }

impl StaticFetch for Memory {
    fn fetch(&self, key: u64) -> impl Future<Output = Option<String>> + Send {
        ready((key == 1).then(|| self.value.clone()))
    }
}

impl DynamicFetch for Memory {
    fn fetch(&self, key: u64) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>> {
        Box::pin(async move { (key == 1).then(|| self.value.clone()) })
    }
}

fn main() {
    let store = Memory { value: "session".into() };
    let mut context = Context::from_waker(Waker::noop());
    let mut concrete = pin!(StaticFetch::fetch(&store, 1));
    assert_eq!(concrete.as_mut().poll(&mut context), Poll::Ready(Some("session".into())));
    let object: &dyn DynamicFetch = &store;
    let mut dynamic = object.fetch(2);
    assert_eq!(dynamic.as_mut().poll(&mut context), Poll::Ready(None));
}
```

StaticFetch 的返回实现由具体类型决定，编译器可静态优化；DynamicFetch 明确拥有一个 boxed Future，付出分配和动态 poll 分派成本。`+'_` 允许借用本次 self，不能直接把它传进要求 static 的后台任务。DynamicFetch 的 Sync 配合共享 self 进入 Send Future，并不说明所有 trait 都必须 Sync。[async/RPIT in traits](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/)、[impl Trait](https://doc.rust-lang.org/reference/types/impl-trait.html)。

async-trait 等工具可以生成类似装箱适配，它们是具体取舍，不是原生 async 的必需前提。本项目 ChatProvider 同时有泛型回调与 async 方法，当前静态接口符合增量设计，不需为了对象化重写。[项目 trait](../../../src/provider/mod.rs)。

## Borrow 和 AsRef 不能混用承诺

Borrow 的相等/排序/哈希行为必须与借用形式一致，HashMap 的借用键查询依赖这项保证。AsRef 主要提供廉价引用转换，不承诺相同的键语义。

```rust
use std::{borrow::Borrow, collections::HashMap};

#[derive(Debug, PartialEq, Eq, Hash)]
struct UserId(String);

impl Borrow<str> for UserId {
    fn borrow(&self) -> &str { &self.0 }
}

fn main() {
    let mut users = HashMap::new();
    users.insert(UserId("u-1".into()), "Alice");
    assert_eq!(users.get("u-1"), Some(&"Alice"));
}
```

本例 newtype 的 Eq/Hash 与内部 String/str 一致。若另做大小写不敏感 Eq/Hash，就不能继续简单地 Borrow 原样 str 并假定查询仍正确。[Borrow 契约](https://doc.rust-lang.org/std/borrow/trait.Borrow.html)、[AsRef](https://doc.rust-lang.org/std/convert/trait.AsRef.html)。

验收：为同一存储接口写静态和动态边界，解释各自借用、Send 和分配；指出一个会重叠的 blanket impl 和一个不满足 Borrow 契约的设计。
