# Design

## Context

见 proposal.md。现有 Agent 包含非 Send 的确认回调；GUI 在自己的线程内创建 Agent 和 current-thread runtime。图形事件、快照、命令展示、授权与关闭逻辑在 ui/gui，React App 直接调用 Tauri。展示历史和读取接口存在 gui 编译条件。前端已有 Vite/tsc/Vitest 和版本 3 的依赖锁文件。

## Goals / Non-Goals

**Goals:** 抽取已存在的图形会话能力，新增第二个宿主；保留 Rust 所有权与业务依赖边界，提供完整单用户多端闭环。

**Non-Goals:** 不改模型/工具循环，不建立插件或多用户框架，不迁移数据库，不强制更换现有 JS 工具的运行时。

## Decisions

1. **组合与所有权**：ui::run 增加 web 分派；ui/app 抽取 GUI 工作线程、命令展示、事件、授权和保存判定。GUI 只保留 Tauri commands 与窗口生命周期，Web 只保留 HTTP/WS 与认证。Agent 在工作线程内创建且不跨线程移动；跨线程仅发送拥有所有权的字符串、快照和结果。错误转换为可展示的文字，不要求业务错误变为 Send。沿用教程的单 Agent 节奏，不使用 actor 框架或共享可变 Agent。
2. **事件与状态**：共用图形事件保留现有正文、工具、用量、快照和诊断语义，增加运行操作和连接来源标识、快照修订号以及连接恢复状态。每个连接使用独立客户端请求编号，服务端分配全局操作编号及 UUID。事件中心在同一锁内注册订阅和发送缓存的完整状态，避免订阅缺口；缓存只保留最新快照、当前流与最多 20 条诊断。普通命令互斥，忙碌即拒绝；状态修订不符即拒绝。授权回复走独立句柄，不能排在等待授权的操作后面。
3. **授权与生命周期**：Web 授权上限 120 秒，任一连接首份回复生效，resolved 事件按授权 ID 清理；全部订阅消失时取消。GUI 保留原窗口关闭、保存失败和强制关闭。Web 的 /exit 走共用保存判定，但关闭失败及退出事件只发到来源页面。删除预览也只发来源，Web 确认携带预览修订号，过期时重新预览；GUI 保留既有显式 /delete --yes 命令，界面按钮继续确认冻结的 UUID。服务信号停止时拒绝新命令、取消授权、排队保存，失败非零退出。
4. **HTTP 与依赖**：可选 web feature 使用 axum 0.8（http1/json/tokio/ws），std 仅提供 TCP，不提供 HTTP、JSON 提取和 WebSocket 升级；使用 include_dir 0.7 嵌入 Vite 哈希资源，std include_bytes 只接受逐文件路径。复用现有 Tokio、futures-util、serde、UUID，显式启用 net/sync/signal 与 sink 能力。测试 WS 客户端用与服务相同版本的 tokio-tungstenite（测试依赖）；std 不提供 WS 握手和帧协议，不手写协议。HTTP 提供 /api/login、/api/logout、/api/auth，/api/ws 提供双向命令和实时事件；不新增轮询或 SSE。最多 16 个连接，每端 256 项有界队列，慢端关闭；入站帧/消息限制 512 KiB，单条输入保留 65536 字节。Ping 15 秒，45 秒无 Pong/响应则断开，发送有时限。
5. **配置与认证**：UiMode 增加 web，WebConfig 仅在选中时解析 GEER_AGENT_WEB_PORT 和 GEER_AGENT_WEB_TOKEN。端口 1..65535、默认 8827，监听固定 0.0.0.0；初始化和 bind 失败明确退出。随机口令使用现有 UUID 随机源，固定口令不打印。登录换取服务端内存随机凭证，HttpOnly/SameSite=Strict/Path=/ Cookie，凭证仅本进程有效。写请求和升级核对 Origin 与合法 Host；静态页面公开，数据和 WS 受认证保护。CSP 只允许本地脚本、样式、图片及本服务 WS，不将模型密钥或访问口令放入事件/前端 env。
6. **前端与 Bun**：src/ui/frontend 为唯一包，packageManager=bun@1.4.2，迁移旧锁保留依赖版本后只提交 bun.lock。依赖入口检查锁文件再 bun install --frozen-lockfile --concurrent-scripts 1；脚本用 bun run，Vite/tsc/Vitest 沿用 Node 兼容运行。Makefile 使用 BUN，可覆写二进制路径；构建配置使用 Node fileURLToPath 兼容 Windows 与空格路径，增加仅编译期的 @types/node 24，不升级原有依赖。GUI/Web 构建使用 Vite 模式别名选择 HostAdapter，分别输出 dist/gui、dist/web。不建立 workspace 或 UI 库发布流程。协议类型集中管理；纯 reducer 和组件复用，只对宿主差异增加适配测试。依赖及测试均进入现有安全入口。
7. **交互与历史**：图形历史条件扩展到 gui/web；模型压缩上下文与完整展示记录继续分离。各端草稿按会话 UUID 存内存，删除当前会话清草稿；多端会话切换不能错投。375px 手机布局提供会话与状态抽屉；Web workspace 为服务器目录输入。复制优先 Clipboard API，HTTP 非安全上下文或拒绝时展示可选择文本。保留中文输入法和 Markdown 安全规则。

## Risks / Trade-offs

- [浏览器连接与 Agent 非 Send] → 仅网络任务持有 Send 通道，Agent 独占工作线程，授权独立应答。
- [断线时操作结果未知] → 自动重连同步完整状态，不自动重发命令；修订号防止陈旧页面修改新会话。
- [仅 HTTP，局域网传输不加密] → 首版遵循用户指定 HTTP；共享口令、同源校验和工具确认限制入口，不实现 TLS 部署体系。
- [GUI 原生依赖与手机布局回归] → 受限 GUI 编译、共用组件回归与原生/浏览器验收。

## Migration Plan

先迁移前端和锁文件并维持 GUI 可构建，再抽取桥接与启用 web，最后加入多端、移动布局和文档。默认 Cargo 不要求 Bun 或 GUI 原生依赖；web/gui 构建分别先准备对应静态产物。更新 Tauri frontendDist、build.rs 提示、Makefile、README、AGENTS 与架构文档。取消 web feature 可回到原入口，无数据迁移。
