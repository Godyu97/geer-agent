# Design

## Context

见 proposal.md 的 Why。当前 `agent::run` 调用 `prompt::load`，把系统提示交给 `Provider`；`repl::run` 只把输入字符串交给 `Agent`。Chat Completions 在 `chat.rs` 保存 `history`，Responses 在 `responses.rs` 保存 `history`、`pending`、`last_output`。前者每次请求插入 system 消息；后者每次请求设置 `instructions`。工具循环已在 `agent`，此变更沿用该循环。

## Goals / Non-Goals

**Goals:** `prompt` 成为系统上下文、已提交历史和当前轮消息的唯一所有者；`repl` 负责将用户输入加载为本轮消息；`provider` 接收现成的请求消息并返回响应数据，保留协议适配职责。

**Non-Goals:** 不实现历史压缩或持久化；不改变工具执行和流式 UI；不把两个 OpenAI 协议的请求结构强行压成有损的纯文本格式。

## Decisions

- **所有权与调用路径。** `agent::run` 仍加载 `Config`、调用现有异步 `prompt::load` 探测环境，然后构造 `Prompt` 和 `Provider`，把 `Prompt` 的可变借用交给 `repl::run`。`repl` 读取普通输入后调用 `Prompt::begin_turn`，再把 `&mut Prompt` 交给 `Session::handle_message`；`/reset` 清空 `Prompt` 的历史和本轮状态，同时调用 session 清除工具授权。`agent` 继续编排最多五轮工具调用，但每一步都从 `Prompt` 取得当前消息快照，再交给 `ChatProvider::complete_step`。这样用户输入的加载入口在 `repl`，而消息拼接只有 `prompt` 一处。

- **请求消息与协议适配。** `Prompt` 按配置中的 `OpenAiApi` 维护带标签的协议消息状态，并提供只读快照：Chat Completions 为首条 system 消息加对话消息；Responses 为 `instructions` 加 `InputItem` 列表。`provider` 持有客户端和模型名，仅把快照设置到请求构造器、附上工具说明、处理流式响应与重试；不保留 system prompt、`history`、`pending`，也不执行消息 `extend`。协议标签在构造时对应配置，若调用方传入错误标签则返回可见错误，不允许静默转换。

- **历史提交规则。** `Prompt` 提供 `apply_tool_results`、`finish_turn`、`commit_turn`、`rollback_turn` 和 `reset`，由现有 `agent` 分支决定何时调用。Chat 分支保持用户输入、助手工具调用和工具结果的既有顺序，缺失的工具调用 id 按原有历史长度规则补齐。Responses 分支保留模型返回的完整 `OutputItem`，连同工具结果一起进入后续输入；不只保存显示文本，因为响应中的工具调用与其他输出项也属于上下文。首次请求失败撤销本轮用户消息；已经执行工具后失败或达到轮次上限则保留已执行工具的消息；成功时保存助手回复。系统上下文始终在独立字段，不随 `/reset` 删除。

- **Rust 映射。** 教程里的 prompt/context/history 对应一个拥有数据的 `Prompt` 结构，而不是在两个模型客户端上各挂一份可变数组。`repl` 与 `agent` 顺序借用它，异步请求期间仅持有已生成的请求快照，避免跨 `.await` 同时可变借用历史。`ChatProvider` 返回 `Result`，协议输出按原始类型带回 `Prompt` 以维持 Responses 的完整性；错误仍由 `agent` 选择提交或回滚。流式回调和空闲超时、重试逻辑留在 `provider`；`Config` 的环境变量读取和缺 key 退出路径不变。只使用现有标准库、`async-openai` 与 Tokio，不新增 crate。

- **不选的方案。** 不让 `provider` 继续保存历史再加一个 `prompt` 缓存，否则会出现双份状态和重置不一致。不让 `repl` 直接组装 OpenAI SDK 的请求结构，否则输入循环要理解两种协议及工具输出。不把 Responses 的原始输出压成统一的文本角色消息，否则后续工具调用可能丢失关联信息。`prompt` 内可以按协议分支转换，但公共的轮次生命周期只定义一份。

## Risks / Trade-offs

- [移动状态时改变 Chat 与 Responses 的提交时机] → 用两种接口的请求体集成测试比较多轮对话、工具结果、失败后继续对话及 `/reset`，再用 `prompt` 单元测试检查状态转移。
- [Responses 的非文本输出丢失] → 保留完整 `OutputItem`，验证工具调用输出在下一步请求中仍可见。
- [协议消息类型使 `prompt` 依赖现有 SDK] → 这是本次保真转换所需的边界；依赖保持在 `prompt` 与 `provider`，不传播到 `repl` 的输入解析或 `tools`。

## Migration Plan

纯进程内重构，没有磁盘数据迁移。按 `Prompt` 状态、provider 请求参数、repl/agent 调用链依次迁移，每一步保持可编译；若验收失败，回退该变更中的模块修改即可。
