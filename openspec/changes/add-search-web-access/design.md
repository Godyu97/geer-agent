# Design

## Context

动机见 proposal。现有 Tools 统一解析、确认与调度，query 通过 bash::run_fixed 使用配置的 Bash，file 提供路径解析，feedback 提供元信息加正文格式。Agent 已将 workspace 注入每个批次，两种 API 与三种 UI 共用该路径。

## Goals / Non-Goals

**Goals:** 增量注册三个工具，复用已有查询执行与结果契约，让网络授权不扩大成任意文件操作授权。

**Non-Goals:** 不更换工具注册表、模型协议、UI 回调或会话数据结构；不搭建通用 MCP 客户端或插件平台。

## Decisions

### 搜索复用受限 rg，而不是另写文件遍历器

在 query 内扩展 Search 分支，解析已有搜索选项和 ignore_case。根与目标使用现有路径展开后 canonicalize，再按 Path 组件检查 strip_prefix；执行前复核路径。固定脚本在规范化 workspace 工作目录执行 rg --json、--no-config、--no-follow、--max-filesize 1M、--sort path，模型参数只经位置参数传入。默认忽略规则与 glob 仍由 rg 决定。

解析完整 JSON 行的 match 事件，按 workspace 输出路径、1 起始行号与未经 trim 的正文；files 去重。每文件最多扫描 51 次命中（第 51 次用于判定截断），stdout 捕获上限 64 KiB。候选行等 end 事件且 binary_offset 为空后，再在阻塞任务中有限检查文件大小与 NUL：rg 的 max-count 可能先于文件后部的二进制标记停止，检查最多读取 1 MiB + 1 字节，不自行复刻忽略规则。检查计入搜索的 10 秒总限时，达到结果上限后不再扫描其他候选文件。捕获尾部不完整或尚未完成文件类型判断时丢弃该尾部并标记截断，不误报协议错误。保留完整命中行，最多 50 项及 2000 字符；诊断优先于部分结果。继承有限 stderr 捕获与进程清理，不额外授权 Bash。教程的逐行扫描映射为 rg 的 match 事件。

### 网页模块持有可复用的异步 HTTP 客户端

增加 web_access，Tools 首次网络调用时构建并持有客户端；准备好的参数拥有 String/URL，避免跨 await 借用临时 JSON。reqwest 0.13 已在锁文件中，直接声明并启用 rustls、json、charset、gzip 和 socks；std 不提供 TLS、HTTP、代理或异步响应读取。客户端设置 geer-agent 的 User-Agent，使用环境代理，保留证书验证；不继承模型密钥、Cookie 或用户认证头。禁用自动重定向，由调用驱动逐步处理。

响应以 chunk 读取并在累计 1 MiB 后停止，不先用无限 text()/bytes()。搜索 25 秒；抓取共享 15 秒网络剩余预算，连接及响应体都在预算内，确认时间不计入。HTML 转换使用 spawn_blocking 并拥有有限响应体，避免堵塞异步运行时。新增 html2text 0.17，std 不解析 HTML 实体、嵌套结构、列表和代码；选择解析器而不是教程的正则剥标签，防止代码中的尖括号与实体被错误清理。JSON、Markdown 等非 HTML 文本直接保留。

### Exa 专用适配与完整来源

固定调用 https://mcp.exa.ai/mcp?tools=web_search_exa 的 tools/call，参数 query、objective=query、numResults；本次探测确认 objective 为必需字段。只解析此服务的 JSON-RPC JSON/SSE 封装与文本来源块，不加入 MCP SDK 或 Day15 注册能力。SSE 按事件组装 data 行，检查匹配 id、error、isError 和文本块。按标题、完整 URL 与摘要整理结果，先收缩摘要再删除超预算的整项，不裁断 URL。无可识别来源但非明确空结果时返回协议错误。

URL 输入限制为 4096 字符，query 为 4096 字符，既符合 objective 上限也为完整 URL 元信息预留输出空间。HTTP(S) 之外、含账号密码的 URL 及未知参数在确认前拒绝。二进制 MIME/含 NUL/无效文本明确失败；文本按响应 charset 解码，默认 UTF-8。网页资料不提升为系统指令。

直接声明 reqwest 的 charset 解码依赖 encoding_rs 0.8：受限 chunk 读取结束后需要独立解码已限制的字节，不能再调用会完整下载响应的 text()。std 只直接支持 UTF-8/UTF-16，不能识别网页常见的其他字符集；此声明复用 HTTP 解码的依赖，不增加另一套编解码实现。

### 授权仍由 Tools 和注入的确认回调控制

search 首次提示规范化 workspace 范围；web_search 首次提示 Exa 和查询，两者沿用 grants/reset。web_fetch 每次确认初始 URL，不缓存授权；同来源重定向自动跟随，scheme/host/effective port 任一变化均再次确认，最多 5 次。网页模块接收一个借用的确认闭包，仅用于重定向，Tools 负责统一提示和拒绝错误。两个网络工具是顺序段边界，search 仍在只读并行段；其他工具行为保持不变。

工具结果仍是 ToolOutput { text, success, changed:false }，错误通过 ToolError 回传。feedback 增加网络专用完整 URL 和工具对应截断提示，现有工具默认格式不变。无需数据库迁移或新增环境配置，GEER_AGENT_TOOLS=off 统一禁用。

## Risks / Trade-offs

- [服务接口或额度变化] → 明确返回 HTTP/RPC/格式错误；mock 固定协议，真实联网作为单独验收，不自动切换服务。
- [静态 HTML 无 JavaScript 正文] → 返回实际抓取文本，说明静态抓取范围，不猜测页面内容。
- [有限查询捕获先于 50 项到达] → 标记截断并保留完整已捕获项，提示收窄范围。
- [路径检查不是操作系统沙箱] → 只给 search 增加规范化、包含检查和不跟随链接；不声称约束已有任意路径工具。

## Migration Plan

新增工具随现有工具开关声明，不改变会话存储。更新简短描述、系统提示和文档。回退时移除三个注册项和新增模块即可，历史工具结果仍作为文本保留。
