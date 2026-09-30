# async-openai Responses API 技术文档

## 1. 概述

`async-openai` 是 Rust 生态中的 OpenAI API 异步客户端库，基于 OpenAI OpenAPI Schema 构建。

本文主要介绍通过 `async-openai` 使用 OpenAI **Responses API** 的方式，包括：

* Client 初始化
* 普通文本请求
* Response 解析
* Streaming
* 多轮上下文
* Function / Tool Calling
* 自定义 Base URL
* 错误处理
* Agent 场景下的推荐架构

本文基于：

```text
async-openai = 0.42.x
```

Responses API 需要启用 `responses` feature：

```toml
[dependencies]
async-openai = { version = "0.42", features = ["responses"] }
```

`responses` feature 会同时启用 Responses API 所需的 API 与 response types。

---

# 2. Cargo 配置

推荐：

```toml
[dependencies]
async-openai = { version = "0.42", features = ["responses"] }

tokio = { version = "1", features = ["full"] }
futures-util = "0.3"

serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

如果只使用非流式 Responses API，则 `futures-util` 不是必须的。

---

# 3. API Key 配置

`async-openai` 默认读取环境变量：

```bash
export OPENAI_API_KEY="sk-xxxx"
```

然后直接创建 Client：

```rust
use async_openai::Client;

let client = Client::new();
```

默认 API Base URL 为：

```text
https://api.openai.com/v1
```

同时支持：

```text
OPENAI_BASE_URL
OPENAI_API_KEY
OPENAI_ORG_ID
OPENAI_PROJECT_ID
```

例如：

```bash
export OPENAI_API_KEY="sk-xxxx"
export OPENAI_BASE_URL="https://api.openai.com/v1"
```

`OpenAIConfig` 默认会读取这些环境变量。

---

# 4. 基础 Responses API 调用

Responses API 的主要入口：

```rust
client.responses()
```

最基本的调用链：

```text
CreateResponseArgs
        │
        ▼
client.responses().create()
        │
        ▼
Response
```

示例：

```rust
use async_openai::{
    Client,
    types::responses::CreateResponseArgs,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    let request = CreateResponseArgs::default()
        .model("your-model")
        .input("你好，请介绍一下 Rust async/await")
        .build()?;

    let response = client
        .responses()
        .create(request)
        .await?;

    println!("{response:#?}");

    Ok(())
}
```

`CreateResponseArgs::input()` 可以直接接受字符串。

内部最终转换为：

```rust
InputParam::Text(String)
```

字符串输入等价于一个 `user` 文本输入。

---

# 5. 获取模型文本输出

Responses API 的返回类型为：

```rust
Response
```

核心字段包括：

```rust
response.id
response.model
response.output
response.usage
response.status
```

其中：

```rust
response.output
```

不是单纯的字符串，而是：

```rust
Vec<OutputItem>
```

因为 Responses API 输出中除了 assistant message，还可能包含：

```text
Message
Reasoning
FunctionCall
WebSearchCall
FileSearchCall
McpCall
CodeInterpreterCall
...
```

`async-openai` 提供了方便方法：

```rust
response.output_text()
```

因此普通文本场景推荐：

```rust
if let Some(text) = response.output_text() {
    println!("{text}");
}
```

完整示例：

```rust
let response = client
    .responses()
    .create(request)
    .await?;

match response.output_text() {
    Some(text) => println!("{text}"),
    None => println!("response does not contain text output"),
}
```

`output_text()` 会遍历 `response.output` 中所有 assistant message，并聚合其中的 `output_text` 内容。

---

# 6. Instructions

可以通过：

```rust
.instructions(...)
```

向模型传递系统级指令。

示例：

```rust
let request = CreateResponseArgs::default()
    .model("your-model")
    .instructions(
        "You are a professional Rust backend engineer. \
         Answer clearly and provide concise code examples."
    )
    .input("解释 Tokio runtime")
    .build()?;
```

推荐理解为：

```text
instructions
    ↓
模型行为 / 系统级约束

input
    ↓
当前用户输入
```

需要注意：

当使用：

```rust
previous_response_id
```

继续上一轮 Response 时，上一轮的 `instructions` 不会自动继承。

如果需要保持同一系统提示词，应在后续请求中继续设置 `instructions`。

---

# 7. 多轮对话

Responses API 支持通过：

```rust
previous_response_id
```

继续上一轮上下文。

第一轮：

```rust
let first = client
    .responses()
    .create(
        CreateResponseArgs::default()
            .model("your-model")
            .input("Rust 的所有权是什么？")
            .build()?,
    )
    .await?;
```

第二轮：

```rust
let second = client
    .responses()
    .create(
        CreateResponseArgs::default()
            .model("your-model")
            .previous_response_id(first.id)
            .input("给我举一个实际例子")
            .build()?,
    )
    .await?;
```

逻辑：

```text
Request #1
   │
   ▼
Response rsp_001
   │
   │ previous_response_id = rsp_001
   ▼
Request #2
   │
   ▼
Response rsp_002
```

这样客户端不需要每次手动发送完整历史 messages。

需要注意：

```text
previous_response_id
```

不能和：

```text
conversation
```

同时使用。

---

# 8. Streaming

对于聊天、Agent、长文本生成等场景，推荐使用 Streaming。

接口：

```rust
client
    .responses()
    .create_stream(request)
```

Responses Streaming 底层使用 SSE：

```text
Server-Sent Events
```

`async-openai` 会将 SSE 解析成：

```rust
ResponseStreamEvent
```

## 8.1 基础 Streaming

```rust
use async_openai::{
    Client,
    types::responses::{
        CreateResponseArgs,
        ResponseStreamEvent,
    },
};

use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    let request = CreateResponseArgs::default()
        .model("your-model")
        .input("详细介绍 Rust Tokio")
        .build()?;

    let mut stream = client
        .responses()
        .create_stream(request)
        .await?;

    while let Some(result) = stream.next().await {
        let event = result?;

        match event {
            ResponseStreamEvent::ResponseOutputTextDelta(event) => {
                print!("{}", event.delta);
            }

            ResponseStreamEvent::ResponseCompleted(event) => {
                println!("\nresponse completed");
                println!("id: {}", event.response.id);
            }

            ResponseStreamEvent::ResponseFailed(event) => {
                eprintln!("response failed: {:?}", event.response.error);
            }

            _ => {}
        }
    }

    Ok(())
}
```

对于普通聊天 UI，最重要的事件通常是：

```rust
ResponseStreamEvent::ResponseOutputTextDelta
```

其作用相当于：

```text
"Rust"
" 是"
" 一门"
" 系统"
" 编程语言"
```

客户端持续拼接：

```text
Rust 是一门系统编程语言
```

Responses Streaming 同时存在大量其他事件，例如：

```text
ResponseCreated
ResponseInProgress
ResponseOutputItemAdded
ResponseOutputTextDelta
ResponseOutputTextDone
ResponseFunctionCallArgumentsDelta
ResponseFunctionCallArgumentsDone
ResponseCompleted
ResponseFailed
ResponseIncomplete
```

---

# 9. Function Calling

Responses API 可以让模型调用应用程序提供的 Tool。

例如定义：

```text
get_weather(city)
```

模型收到：

```text
洛杉矶天气怎么样？
```

可能产生：

```text
FunctionCall
    name = get_weather
    arguments = {"city":"Los Angeles"}
```

程序执行 Tool 后，将结果再次发送给模型。

完整流程：

```text
User
 │
 ▼
Responses API
 │
 ▼
FunctionCall
 │
 ▼
Local Tool Executor
 │
 ▼
FunctionCallOutput
 │
 ▼
Responses API
 │
 ▼
Final Answer
```

---

# 10. 定义 Function Tool

`async-openai` 使用：

```rust
FunctionTool
```

描述本地函数。

例如：

```rust
use async_openai::types::responses::{
    FunctionToolArgs,
    Tool,
};

use serde_json::json;

let weather_tool = FunctionToolArgs::default()
    .name("get_weather")
    .description("Get current weather for a city")
    .parameters(json!({
        "type": "object",
        "properties": {
            "city": {
                "type": "string",
                "description": "City name"
            }
        },
        "required": ["city"],
        "additionalProperties": false
    }))
    .strict(true)
    .build()?;

let tool = Tool::Function(weather_tool);
```

然后放入 Request：

```rust
let request = CreateResponseArgs::default()
    .model("your-model")
    .input("洛杉矶天气怎么样？")
    .tools(vec![tool])
    .build()?;
```

Function Tool 的参数 Schema 使用 JSON Schema。`FunctionTool` 包含 `name`、`description`、`parameters`、`strict` 等字段。

---

# 11. 处理 FunctionCall

模型请求调用本地函数时：

```rust
response.output
```

中会出现：

```rust
OutputItem::FunctionCall(...)
```

示例逻辑：

```rust
use async_openai::types::responses::OutputItem;

for item in &response.output {
    if let OutputItem::FunctionCall(call) = item {
        println!("tool name: {}", call.name);
        println!("call id: {}", call.call_id);
        println!("arguments: {}", call.arguments);
    }
}
```

`FunctionToolCall` 的主要字段：

```rust
pub struct FunctionToolCall {
    pub arguments: String,
    pub call_id: String,
    pub name: String,
    ...
}
```

注意：

```rust
arguments
```

是 JSON 字符串，而不是已经解析好的 Rust Struct。

因此通常需要：

```rust
serde_json::from_str(...)
```

例如：

```rust
#[derive(serde::Deserialize)]
struct WeatherArgs {
    city: String,
}

let args: WeatherArgs =
    serde_json::from_str(&call.arguments)?;
```

---

# 12. Tool 执行循环

对于 Agent，推荐实现统一 Tool Loop：

```text
loop
 │
 ├─ responses.create()
 │
 │
 ├─ Message
 │     └─ 返回最终文本
 │
 └─ FunctionCall
       │
       ▼
    execute_tool()
       │
       ▼
    FunctionCallOutput
       │
       └───────────────┐
                       │
                       ▼
                 responses.create()
```

伪代码：

```rust
loop {
    let response = client
        .responses()
        .create(request)
        .await?;

    let mut tool_calls = Vec::new();

    for item in &response.output {
        if let OutputItem::FunctionCall(call) = item {
            tool_calls.push(call);
        }
    }

    if tool_calls.is_empty() {
        return Ok(response.output_text());
    }

    for call in tool_calls {
        let output = execute_tool(
            &call.name,
            &call.arguments,
        ).await?;

        // 将 call_id + output
        // 作为 FunctionCallOutput
        // 发送给下一轮 Responses API。
    }
}
```

---

# 13. Agent 场景中的一个重要细节

如果选择手动维护 `input`，而不是使用：

```rust
previous_response_id
```

那么 Tool Calling 时不要只把：

```text
FunctionCall
FunctionCallOutput
```

发回模型。

对于包含 reasoning item 的模型，需要同时保留模型上一轮产生的相关：

```text
Reasoning
```

输出。

`async-openai 0.42` 已实现：

```rust
From<OutputItem> for InputItem
```

因此可以把上一轮：

```rust
response.output
```

直接转换后追加到下一轮 input。

这是 Agent 实现中比较重要的一点，因为只回传 function call 而丢失对应 reasoning item，可能导致服务端拒绝请求。

推荐：

```text
input
  +
response.output
  +
function_call_output
```

而不是：

```text
input
  +
function_call_output
```

---

# 14. previous_response_id 与手动 Context

有两种上下文维护方案。

## 方案 A：previous_response_id

```rust
.previous_response_id(response.id)
```

优点：

```text
实现简单
代码量少
适合普通聊天
适合简单 Agent
```

流程：

```text
rsp_1
 ↓
rsp_2
 ↓
rsp_3
```

---

## 方案 B：手动维护 InputItem

应用层自己维护：

```rust
Vec<InputItem>
```

形式：

```text
User Message
Reasoning
FunctionCall
FunctionCallOutput
Assistant Message
User Message
...
```

优点：

```text
上下文完全可控
方便裁剪 Context
方便持久化
方便迁移模型
方便实现自定义 Agent Runtime
```

如果是在实现真正的 Agent 框架，通常建议使用：

```text
手动 Context
```

如果只是业务系统调用模型，优先：

```text
previous_response_id
```

---

# 15. 自定义 OpenAI-compatible API

如果使用 Gateway、Proxy 或兼容 OpenAI Responses API 的第三方服务，可以修改 Base URL。

```rust
use async_openai::{
    Client,
    config::OpenAIConfig,
};

let config = OpenAIConfig::new()
    .with_api_key("your-api-key")
    .with_api_base("https://llm.example.com/v1");

let client = Client::with_config(config);
```

后续调用完全相同：

```rust
let response = client
    .responses()
    .create(request)
    .await?;
```

也可以直接通过环境变量：

```bash
export OPENAI_API_KEY="xxx"
export OPENAI_BASE_URL="https://llm.example.com/v1"
```

需要注意，第三方服务即使声明：

```text
OpenAI Compatible
```

也不代表完整支持 Responses API。

很多兼容服务实际上只实现：

```text
/v1/chat/completions
```

而没有实现：

```text
/v1/responses
```

因此需要确认供应商支持的具体 endpoint 和 event schema。

---

# 16. create / create_stream

`Responses` 主要提供：

```rust
create()
create_stream()

retrieve()
retrieve_stream()

delete()
```

普通请求：

```rust
let response = client
    .responses()
    .create(request)
    .await?;
```

Streaming：

```rust
let stream = client
    .responses()
    .create_stream(request)
    .await?;
```

获取已存储 Response：

```rust
let response = client
    .responses()
    .retrieve("resp_xxx")
    .await?;
```

删除：

```rust
client
    .responses()
    .delete("resp_xxx")
    .await?;
```

---

# 17. 错误处理

API 调用返回：

```rust
Result<Response, OpenAIError>
```

推荐业务层不要直接：

```rust
.unwrap()
```

应该：

```rust
match client.responses().create(request).await {
    Ok(response) => {
        // success
    }

    Err(err) => {
        tracing::error!(
            error = ?err,
            "openai responses request failed"
        );
    }
}
```

工程中建议进一步定义业务 Error：

```rust
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("OpenAI request failed: {0}")]
    OpenAI(#[from] async_openai::error::OpenAIError),

    #[error("invalid tool arguments: {0}")]
    InvalidToolArguments(#[from] serde_json::Error),

    #[error("tool execution failed: {0}")]
    Tool(String),
}
```

这样可以把：

```text
Transport Error
API Error
Tool Error
JSON Error
Agent Error
```

进行分层。

---

# 18. 推荐项目结构

对于普通业务：

```text
src/
├── llm/
│   ├── mod.rs
│   ├── client.rs
│   └── responses.rs
│
└── main.rs
```

对于 Agent：

```text
src/
├── llm/
│   ├── client.rs
│   └── responses.rs
│
├── agent/
│   ├── agent.rs
│   ├── context.rs
│   ├── runtime.rs
│   └── stream.rs
│
├── tools/
│   ├── mod.rs
│   ├── registry.rs
│   ├── executor.rs
│   └── weather.rs
│
└── main.rs
```

推荐职责：

```text
OpenAIClient
    │
    ├── API 请求
    └── API 配置

AgentRuntime
    │
    ├── Context 管理
    ├── LLM Loop
    └── Tool Loop

ToolRegistry
    │
    ├── Tool Schema
    └── Tool 查找

ToolExecutor
    │
    └── 实际执行 Tool
```

不要把：

```text
HTTP 请求
Tool 执行
Context 管理
业务逻辑
```

全部写在一个函数中。

---

# 19. 推荐 Agent Runtime

完整架构建议：

```text
                 ┌───────────────┐
                 │     User      │
                 └───────┬───────┘
                         │
                         ▼
                 ┌───────────────┐
                 │ Agent Runtime │
                 └───────┬───────┘
                         │
                         ▼
                 ┌───────────────┐
                 │ Responses API │
                 └───────┬───────┘
                         │
              ┌──────────┴──────────┐
              │                     │
              ▼                     ▼
         Text Output          Function Call
              │                     │
              │                     ▼
              │             ┌───────────────┐
              │             │ Tool Registry │
              │             └───────┬───────┘
              │                     │
              │                     ▼
              │             ┌───────────────┐
              │             │ Tool Executor │
              │             └───────┬───────┘
              │                     │
              │             Function Output
              │                     │
              │                     ▼
              │              Responses API
              │                     │
              └──────────────┬──────┘
                             ▼
                        Final Answer
```

Agent Runtime 应该负责判断：

```text
Response
    │
    ├── 有 FunctionCall
    │       ↓
    │    执行 Tool
    │       ↓
    │    再次请求模型
    │
    └── 无 FunctionCall
            ↓
         返回最终 Answer
```

---

# 20. Streaming + Tool Calling

对于 Agent，不建议把 Streaming 理解成单纯的：

```text
文字逐 token 输出
```

Responses API 的 Streaming 实际上传输的是：

```text
Response Event Stream
```

例如：

```text
response.created

response.output_text.delta

response.function_call_arguments.delta

response.function_call_arguments.done

response.output_item.done

response.completed
```

因此 Agent Streaming 推荐：

```rust
match event {
    ResponseStreamEvent::ResponseOutputTextDelta(e) => {
        // 推送文本给客户端
    }

    ResponseStreamEvent::ResponseFunctionCallArgumentsDelta(e) => {
        // 累积 Tool Arguments
    }

    ResponseStreamEvent::ResponseFunctionCallArgumentsDone(e) => {
        // Tool 参数完成
    }

    ResponseStreamEvent::ResponseCompleted(e) => {
        // 当前 Response 完成
    }

    _ => {}
}
```

因此在 Agent 系统中：

```text
LLM Stream
```

依然很有意义。

它不仅用于用户看到文字逐步生成，也用于尽早获得：

```text
FunctionCall
Function Arguments
Response State
```

---

# 21. 推荐封装

业务层不建议直接大量依赖：

```rust
async_openai::types::responses::*
```

推荐增加一层 Adapter：

```rust
pub struct LlmClient {
    client: async_openai::Client<OpenAIConfig>,
    model: String,
}
```

例如：

```rust
impl LlmClient {
    pub async fn generate(
        &self,
        input: &str,
    ) -> Result<String, LlmError> {
        let request = CreateResponseArgs::default()
            .model(&self.model)
            .input(input)
            .build()?;

        let response = self
            .client
            .responses()
            .create(request)
            .await?;

        Ok(response
            .output_text()
            .unwrap_or_default())
    }
}
```

业务代码最终只依赖：

```rust
llm.generate(...)
```

而不是依赖具体 SDK。

这样未来切换：

```text
async-openai
OpenAI SDK
自建 Gateway
其他 Provider
```

时成本更低。

---

# 22. 总结

`async-openai` Responses API 的核心调用方式可以总结为：

```text
Client
  │
  ▼
CreateResponseArgs
  │
  ▼
responses().create()
  │
  ▼
Response
  │
  ├── Message
  ├── Reasoning
  ├── FunctionCall
  └── Other OutputItem
```

普通文本调用：

```rust
let response = client
    .responses()
    .create(
        CreateResponseArgs::default()
            .model("your-model")
            .input("hello")
            .build()?,
    )
    .await?;

let text = response.output_text();
```

Streaming：

```rust
client
    .responses()
    .create_stream(request)
    .await?;
```

多轮：

```rust
.previous_response_id(response.id)
```

Tool Calling：

```text
Response
   ↓
FunctionCall
   ↓
execute_tool()
   ↓
FunctionCallOutput
   ↓
Response
```

对于普通 LLM API 接入，推荐：

```text
Responses API
+
previous_response_id
```

对于复杂 Agent Runtime，推荐：

```text
Responses API
+
Streaming
+
手动 InputItem Context
+
Tool Registry
+
Tool Executor
+
统一 Agent Loop
```

这套设计可以比较自然地支持后续的：

```text
多 Tool
并行 Tool Calling
MCP
Reasoning Model
Streaming
Context 裁剪
Agent 状态机
多模型 Provider
```
