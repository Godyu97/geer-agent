# E6 Serde 与 Reqwest：类型化边界与可控 HTTP 客户端

[返回总目录](../README.md) · [上一篇](05-axum-and-tower.md) · [下一篇](07-database-and-rpc.md)

## Serde 的分层

Serde 定义 Serialize/Deserialize 与数据模型，derive 生成类型适配；serde_json 负责 JSON 格式，其他格式 crate 可以使用同一套类型契约。JSON 解析只是协议形状检查，不能代替业务范围验证。[Serde 数据模型](https://serde.rs/data-model.html)、[derive](https://serde.rs/derive.html)。

```mermaid
flowchart LR
    A[输入字节] --> B[格式解析器 serde_json]
    B --> C[Deserialize 到 DTO]
    C --> D[业务验证与领域类型]
    D --> E[业务处理]
    E --> F[响应 DTO Serialize]
    F --> G[JSON 字节]
```

领域对象、数据库模型和 API DTO 可以适当分开，避免把数据库字段与机密状态顺手全部暴露；小练习没有必要为了分层复制几十个同形类型。

## 配置属性如何影响兼容性

| 属性 | 用途 | 要说明的边界 |
| --- | --- | --- |
| rename / rename_all | Rust 命名与协议命名分开 | 序列化和反序列化两侧可有区别 |
| alias | 接受旧字段名 | 不意味着输出同时含多个名字 |
| default | 缺字段时建立默认值 | 默认值是否具有正确业务含义 |
| skip_serializing_if | 省略某种输出 | 输入规则需另行设计 |
| deny_unknown_fields | 拒绝未知字段 | 更严格，前向兼容性也更低 |
| flatten | 展开嵌套结构 | 与 deny_unknown_fields 有兼容限制 |
| tag / content / untagged | 控制 enum 表示 | 歧义、错误信息与扩展策略 |

细节以 [container attributes](https://serde.rs/container-attrs.html)、[field attributes](https://serde.rs/field-attrs.html)、[enum representations](https://serde.rs/enum-representations.html) 为准。尤其不能把旧数据缺字段、新字段为 null 和合法默认值混成一个行为。

## 一个完整 JSON 协议练习

独立 Cargo 包添加 serde derive 和 serde_json：

```rust,ignore
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Event {
    Started { id: String },
    Finished { id: String, output: String },
    Failed { id: String, code: String },
}

fn main() -> Result<(), serde_json::Error> {
    let event = Event::Started { id: "job-1".into() };
    let json = serde_json::to_string(&event)?;
    let restored: Event = serde_json::from_str(&json)?;
    assert_eq!(event, restored);
    assert!(json.contains("\"kind\":\"started\""));
    Ok(())
}
```

练习新增 Cancelled 变体并讨论旧客户端收到它时的行为。数据库回放、WebSocket 和 Tauri Channel 使用同一种类型时，版本兼容必须与多端部署节奏一起考虑。

## 借用反序列化与 DeserializeOwned

从仍存活的输入字符串解析，可以设计带借用字段的 DTO；从读取器或会自行释放输入的接口取得结果，通常要求拥有的数据。`DeserializeOwned` 说明结果不依赖某一次输入借用；把 `'static` 乱加到字段并不能让临时输入变得永远有效。[Serde lifetimes](https://serde.rs/lifetimes.html)。

serde_json::Value 适合未知或灵活协议边界，稳定业务结构用具名类型更方便校验与维护；Value 不会自动验证必需字段、版本和范围。[serde_json](https://docs.rs/serde_json/latest/serde_json/)。

## Reqwest 在调用链中的位置

Client 管连接池、请求配置与协议传输，RequestBuilder 构造请求，Response 承载状态、头和可流式读取的 body。Client 适合复用，其 clone 共享内部状态；每个请求新建 Client 会损失连接复用机会。[Client](https://docs.rs/reqwest/latest/reqwest/struct.Client.html)。

本项目使用 Reqwest 0.13，因此下面 feature 写 rustls；旧版本常见的 rustls-tls 配置需按版本确认，不要照抄。

```toml
[dependencies]
reqwest = { version = "0.13", default-features = false, features = ["rustls", "json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt", "time"] }
```

## 完整客户端：请求前文的本机计数器

```rust,ignore
use std::{error::Error, io, time::Duration};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct Increment { amount: u32 }
#[derive(Deserialize)]
struct Snapshot { value: u32 }

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(2))
        .build()?;
    let mut response = client.post("http://127.0.0.1:3000/counter")
        .json(&Increment { amount: 1 })
        .send().await?
        .error_for_status()?;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if chunk.len() > 16_384_usize.saturating_sub(body.len()) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "response too large").into());
        }
        body.extend_from_slice(&chunk);
    }
    let snapshot: Snapshot = serde_json::from_slice(&body)?;
    println!("counter={}", snapshot.value);
    Ok(())
}
```

这是完整客户端程序，但运行需要前文服务；编译检查和真实 HTTP 交互应分别记录。累计 body 有上限，不表示库内部每个网络 chunk、解压状态或解析开销都严格等于同一额度；外层仍需总资源限制。[Response](https://docs.rs/reqwest/latest/reqwest/struct.Response.html)。

## 调用失败至少分四层

| 层 | 例子 | 常见处理 |
| --- | --- | --- |
| 传输 | DNS、连接、TLS、超时 | 根据幂等与剩余预算决定重试 |
| HTTP | 401、429、503 | 状态码与 Retry-After 策略 |
| 解码 | 非 JSON、字段类型错 | 作为协议错误保留脱敏诊断 |
| 业务 | JSON 中返回 conflict | 按领域规则处理 |

send 成功不自动将所有 4xx/5xx 当 Rust Err，error_for_status 提供状态检查。总 deadline、连接超时和流中空闲超时解决不同问题，重试不能每轮重置整个预算。[ClientBuilder](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html)。

## 重定向、代理、TLS 与重试

抓取用户提供 URL 时，限制协议、目标网络范围、重定向每一跳、解析后的地址、超时和输出量；只检查最初 URL 不够。代理会改变连接目标与信任边界，生产配置需明确继承哪些环境。不要为排查证书问题长期关闭验证。[Reqwest redirect](https://docs.rs/reqwest/latest/reqwest/redirect/index.html)、[Proxy](https://docs.rs/reqwest/latest/reqwest/struct.Proxy.html)。

客户端自身可能包含协议层重试行为；叠加业务重试前核对版本和配置。涉及创建、扣费、工具执行等操作，只有业务幂等或能确认未提交时才可安全重复。项目的 [Responses 重试](../../../src/provider/openai/responses.rs) 与 [web_access](../../../src/tools/web_access.rs) 提供不同边界的实例。

验收：对同一个 URL 模拟 200 非 JSON、503、超大 body、超时和重定向，分别给出准确错误；测试只用有时限的本机服务。
