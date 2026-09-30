# Design

## Context

见 proposal.md。LLM/工具编排已经位于 Agent，当前需要抽取的是 REPL、TUI、GUI 各自重复的会话命令执行。三种界面已共用 Input 解析和 Session 契约；GUI commands 同时承担业务分派与窗口结果包装，文本循环则直接调用各种 Session 操作。

现有会话操作部分返回 String，删除已返回结构化预览/报告。界面布局、错误流和确认方式各不相同，TUI 的会话面板读取结构化列表，其他界面的 /sessions 使用已有文本列表。现有主 spec 的“未配置数据库”叙述与代码中的默认 SQLite 策略有历史差异；本重构沿用运行行为，不调整这项策略。

## Goals / Non-Goals

**Goals:** 在中立模块收敛命令与消息执行；文本 UI 位于 ui/repl，只承担输入、展示与交互确认；各界面复用 Agent 功能；工具核心不直接操作终端。

**Non-Goals:** 不设计通用命令总线，不改造模型流的字符串回调，不迁移存储格式，不一次性修改所有会话返回类型，不合并不同界面的展示状态。

## Decisions

### 共用执行入口留在 interaction

增加 interaction::execute，借用 &mut impl Session，接收解析后的 Input 以及现有正文/用量回调。它统一调用消息、新建、保存、压缩、打开、workspace、列表与删除操作，返回 CommandOutcome；失败通过包含操作类别与原因的 CommandError 返回。

结果区分新建（保留 reset 标记）、打开、保存、列表、删除预览/报告、普通消息完成及退出请求，不含 ANSI、窗口或布局。错误前缀等纯展示内容由 UI helper 生成。Agent 继续实现 Session，execute 不持有另一份会话状态，不导入具体 Agent、Provider 或 Tools。

这是现有能力上的普通 Rust 函数和枚举，不增加框架或 crate。将执行入口放到 ui 会让业务功能依附具体界面；另建一套 Agent 控制器或复制现有模型循环则会增加状态与历史维护成本。当前保留泛型异步接口，不改为 dyn Session。

### 三种 UI 保留自己的适配逻辑

REPL 读取输入、调用解析器和 execute，再渲染结果；处理 EOF 时退出输入循环，通过统一保存操作补写并输出 bye。保持 /reset 特有文案、Session ID 单独成行、stdout/stderr 选择和流式 flush。

TUI 的普通消息及会话写操作调用 execute。面板键盘动作转换为同样的 Input；读取 status/session_entries 等绘图数据仍直接使用中立 Session 契约。面板打开、刷新、草稿与焦点是 UI 状态，不进入共用执行函数。

GUI commands 保留窗口结果包装和流式工具标记识别，只把 execute 的结果映射到现有 CommandResult。Bridge 的提交/关闭、授权应答、request ID 与快照仍按原线程模型运行，关闭保存使用共用保存入口。

### 删除保留两阶段确认

未确认的删除只返回预览，明确确认后才执行。共用入口校验并去重完整 UUID，避免面板直接构造 Input 时绕过参数约束。UI 决定如何向用户询问；文本 UI 在非交互且没有 --yes 时提前拒绝，不读取下一行作为确认。TUI/GUI 继续使用已有面板/弹窗，不改选中集合、失败重试与刷新行为。

### 文本 UI 与终端工具授权整体迁移

顶层 repl 迁为 ui/repl，移除 main 的顶层 mod repl；颜色保持文本界面私有。命令解析测试归回 interaction，UTF-8、EOF、确认输入等测试归文本 UI。

从 Tools 移出 confirm_cli 的 stdin/stdout 操作，放到 ui/repl/authorization；Tools 默认确认回调拒绝。ui::run 在文本路径通过 Agent::set_confirm 注入它，TUI/GUI 沿用已有注入。仍使用 FnMut(&str) -> io::Result<bool>，保留授权范围、缓存和撤销规则。

### 所有权、错误与异步

Agent 和各会话的所有权不变。execute 对 Session 的可变借用只覆盖一条操作；结果拥有 String 或现有预览/报告，不向 UI 泄漏模型协议对象。流式闭包同步调用，正文和用量顺序、错误返回语义保持，GUI 在已有桥接边界复制文本，不跨线程搬迁 Agent。

回调失败与业务失败保留原错误原因，由 UI 对操作类别选择既有前缀。终端 I/O 错误继续以 Result 向上返回，TUI 恢复守卫与 GUI 关闭失败处理继续生效。环境加载、界面选择和模型配置均不改变。

## Risks / Trade-offs

- [统一执行时意外统一 UI 文案] → 结果提供操作语义，由各界面保留原文案；用现有跨进程回归核对输出。
- [删除预览被误当作同意] → 未确认仅预览；直接构造的删除请求也校验 UUID；验证取消与非交互输入不发生删除。
- [默认拒绝后漏装文本授权回调] → 组合入口显式安装，保留非交互拒绝回归，核对三条 UI 启动路径。
- [抽取后绕过流式错误或失去用量顺序] → 使用假 Session 钉住增量/用量顺序与错误传播，并运行两种 API 的集成测试。
- [GUI feature 构建漏检] → 默认测试仍编译无 Tauri 的 GUI commands 测试；再在受限服务中检查实际 gui feature，单独记录原生窗口验收条件。

## Migration Plan

先增加共用执行入口和测试，再逐个接入 REPL、GUI、TUI，最后移走终端授权与同步文档。每一步沿用已有 Agent，不迁移用户数据。回退时恢复 UI 调用及旧目录，不涉及数据库或配置回滚。
