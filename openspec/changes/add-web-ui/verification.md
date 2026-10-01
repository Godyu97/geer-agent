# Web UI 实现验证（2026-10-01）

## 交付内容

- 可选 `web` feature 与 `GEER_AGENT_UI=web`，固定监听 `0.0.0.0`；默认端口 9928，`GEER_AGENT_WEB_PORT` 可配置。`make web` 构建前端后启动，`make web-build` 生成嵌入静态资源的 release 产物。
- 口令登录、内存 Cookie 凭证、同源写入和 WebSocket 校验；单 Agent 多浏览器共享会话、实时状态与授权，操作串行并校验 revision。
- `ui/app` 共用工作线程、命令、缓存状态、授权和保存判定；GUI 保留 Tauri 窗口与原生能力，Web 保留 HTTP/WS 传输与认证。
- 唯一前端包 `src/ui/frontend`：共用 App、reducer、协议与 Mocha 样式，构建时选择 HostAdapter，分别输出 `dist/gui` 和 `dist/web`。提供手机抽屉、会话 UUID 草稿和 HTTP 手动复制。
- Bun 1.4.2 管理依赖、锁文件和脚本。旧锁中的 290 项依赖版本全部保留，仅新增 `@types/node@24.19.0` 与其 `undici-types@7.24.6`，用于构建配置的 Node 类型。`fileURLToPath` 处理 Windows 与空格路径，Vite/tsc/Vitest 保持兼容 Node 的运行方式。README、AGENTS、OpenSpec 项目背景与架构文档统一记录共用前端维护边界。

## 自动检查

| 命令 / 检查 | 结果 |
| --- | --- |
| `make check` | fmt、安全探针、默认 Rust 全量测试与 Clippy 顺序通过；238 项通过，1 项真实网络手动测试按原约定忽略 |
| `make test FEATURES=gui` | GUI feature 全量 Rust 回归通过 |
| `make test FEATURES=web` | Web feature 全量 Rust 回归通过 |
| `make test FEATURES=web TEST=ui::app::authorization` | 最新授权缓存修复的 5 项回归通过 |
| `make test FEATURES=web TEST_ARGS='--test web_ui'` | 最新实现的 3 项实际 HTTP/WS 集成测试通过 |
| `make frontend-test` | 4 个文件、30 项共用 GUI/Web 前端测试全部通过 |
| `make clippy-all` | Bun 冻结安装、GUI/Web 类型检查与构建、`gui,web` 全目标 Clippy 通过，无新警告 |
| `scripts/test-safe.sh cargo check --features desktop-gui` | 桌面入口编译通过 |
| Web bundle 检查 | 无 `__TAURI__`、`gui_submit` 或 `ipc.localhost` |
| `openspec validate add-web-ui --strict` | 通过 |
| `git diff --check` | 通过 |

授权回归与 HTTP/WS 回归在最后一次授权缓存修复后单独运行；后续 GUI 全量和默认全量检查也覆盖该修复。最后的构建配置类型补充经过两种前端构建、30 项前端回归和全目标 Clippy 检查。

所有测试均使用项目受限入口，先验证实际 cgroup 与私有 tmpfs 隔离，不并发叠加测试组。常规额度为内存 4G、无 swap、任务 256、总时限 10 分钟；Rust 编译 2 并行、测试 1 线程，Vitest 固定 1 worker 并关闭文件并行。安全探针验证正常、失败、SIGKILL、超时、有限 OOM 及启动器中断后的回收。

## 行为覆盖

实际 HTTP/WS 测试只连接本机限时模型夹具，工作区、SQLite 与工具夹具均在本轮私有临时目录中，不使用真实模型密钥或用户会话：

- 未认证读取与升级拒绝，正确/错误口令、Cookie 属性、跨源请求、非法 Host、注销与凭证失效；事件不包含模型密钥或访问口令。
- 生成途中加入恢复历史、输入和已有增量；并发与旧 revision 提交拒绝；任一端首份工具授权生效，迟到回复失效，全部端断开默认拒绝且重连不恢复已解决授权。
- 会话切换、workspace、私有删除预览、过期确认拒绝、当前会话删除后的空会话；输入 65536 字节、消息/帧 512 KiB 和第 17 个连接拒绝。
- `/exit` 保存并仅离开来源端，其他端继续执行；仅内存会话显示真实保存失败并可返回；SIGTERM 成功保存退出，保存失败返回非零状态。
- 非法端口、监听占用、缺少模型配置明确失败。

前端回归保留原 GUI 的中文输入法、Markdown 安全、历史、批量管理、删除失败选择和关闭行为，并加入草稿随远端切换保留、重连恢复、忙碌请求失败不清除他端运行状态、revision、断线禁止提交、不自动重发、登录口令不存储和手动复制。

## 浏览器验收

使用一次性受限服务及虚构模型配置，在 Chrome 验证登录、中文聊天、Markdown、连接状态和完整历史。桌面布局与 GUI 保持 Mocha 风格；375 CSS 像素宽度下页面无横向溢出，会话与状态抽屉可操作。通过局域网 HTTP 地址验证非安全上下文的复制回退：弹出可选择文本并自动选中代码。未观察到浏览器控制台错误或警告。

- [桌面截图](screenshots/web-desktop.png)
- [375px 手机截图](screenshots/web-mobile.png)
- [HTTP 手动复制截图](screenshots/web-copy.png)

验收结束已关闭临时浏览器标签、恢复视口，并停止本轮命名测试服务；临时文件由隔离服务回收。未做原生 GUI 窗口手动点击或 Windows 交叉编译；GUI 兼容性由共用前端回归、GUI Rust 全量回归及桌面 feature 编译检查验证。

接入架构与后续前端管理建议见 [架构文档](../../../docs/design/architecture.md)；启动与配置见 [README](../../../README.md)。
