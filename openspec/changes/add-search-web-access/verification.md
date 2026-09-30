# 验收记录

## 已执行

- OpenSpec 中文规划产物通过 `openspec validate add-search-web-access --strict`。
- 受限编译及注册表单项通过，声明 11 个工具；三个新工具复用原调用链。
- 修改查询捕获预算后，顺序通过 `make test-safety`、缺失命令单项和 `process_safety` 四项回归；实际 cgroup 内存和任务限制、禁用 swap、独立 tmpfs 均生效。
- 搜索单项通过：选项组合、行号和缩进、子目录路径接 read、忽略和显式覆盖、大小/二进制过滤（含文件后部 NUL 与混合无效 UTF-8）、相对/绝对越界、同名前缀、符号链接、路径编码错误、workspace 切换、特殊参数、非法正则、50 项及完整行截断。
- `tools::web_access::tests` 的八项自动测试通过：Exa JSON/多行 SSE 与通知、query/objective、完整 URL、RPC/403/429/协议错误、无结果、定长/分块响应预算、累计网络超时与取消连接、HTML/文本/编码、二进制拒绝、逐次/重定向确认和拒绝不发请求。
- 真实联网手动单项 `make test TEST_ARGS='--bin geer-agent real_web_access_smoke -- --ignored --nocapture'` 使用实现后的 Rust 客户端通过：Exa 返回 2 项；抓取 `https://doc.rust-lang.org/std/fs/fn.read_to_string.html` 返回 HTTP 200、text/html、6262 字节，没有重定向。该测试默认忽略，公网服务不参与自动质量门。

## 界面展示与复查步骤

三种界面继续使用同一个确认回调。两种模型 API 的成功调用与结果配对、非交互 REPL 拒绝/工具关闭均通过；`tool_loop` 的 29 项集成回归通过。TUI TestBackend 验证完整 URL 在窄/宽界面换行；GUI 前端 23 项测试和类型检查通过，授权弹窗保留完整提示并默认拒绝。

另在受限服务中通过本机 HTTP 模拟服务和 Python 标准库伪终端，实际启动 REPL 与 TUI 并发送键盘输入。两条路径都注册 11 个工具，完成一次首次 search 授权、两次 web_fetch 的逐次授权、两次跨端口重定向的再次授权，随后收到成功结果并正常退出。服务内存峰值 25 MiB，输入、输出、网络和运行时间均有固定上限。

本次无法通过工具操作桌面原生 GUI 窗口；前端自动测试不等同于原生窗口人工验收。以下步骤用于重复终端验收和补充 GUI 人工检查。

1. 在交互 REPL 或 TUI 中要求搜索 workspace 中已有文本，检查首次提示的根目录和范围；随后要求 read 命中文件，检查相对路径和上下文；再次 search 不应重复询问。
2. 要求 web_search 查询公开内容，检查 Exa 和完整查询提示，拒绝后不发送；允许后可继续使用同一工具，reset 后重新询问。
3. 要求两次 web_fetch 同一静态 HTTP(S) 地址，检查每次都出现完整 URL；本机页面返回指向另一个端口的 302 时，再确认目标 URL，拒绝后目标服务不能收到请求。同来源重定向无需额外确认。
4. 以 `GEER_AGENT_UI=gui make gui` 启动 GUI，重复上一步；长 URL 的提示区可以换行及滚动，通用文案不会承诺逐次工具缓存授权。关闭窗口或拒绝授权后，不应继续访问目标。

## 最终质量门

- 最终 `make check` 通过：fmt、安全入口八种回收探针、231 项 Rust 测试（另 1 项真实联网单项默认忽略）、默认 `--all-targets` clippy，无警告。完整测试服务内存峰值 1.4 GiB，swap 为 0；隔离设置保持默认 4 GiB / 256 任务 / 10 分钟。
- `make gui-test` 的 23 项前端测试与受限 `make gui-check` 通过；`make clippy-all` 通过，前端生产构建及实际 `--features gui` Rust 代码检查完成，无警告。GUI Rust 检查服务内存峰值 1.5 GiB。
- `openspec validate add-search-web-access --strict` 与 `git diff --check` 通过。
- 真实 Exa 和静态网页验收、REPL/TUI 伪终端验收已单独执行；原生 GUI 窗口的键盘/鼠标人工检查仍按上文步骤补充。
