# Tasks

## 1. 终端状态

- [x] 1.1 修复本轮和累计的流式估算，只计算新增输出；用 `cargo test ui::tui` 覆盖非零上下文、奇数字符及 usage 替换。
- [x] 1.2 在命令和会话面板共用的状态同步中清理成功切换后的消息、草稿及滚动，失败和同会话保持原状态；用 `cargo test ui::tui` 验证显示及状态隔离。

## 2. 检查点

- [x] 2.1 SQL 事件及检查点使用同一事务并保留相同事件重试；用 `cargo test dao` 验证检查点冲突、事件冲突回滚及幂等。
- [x] 2.2 保存和恢复上下文校正，移除恢复后的强制提交，压缩先清零再保存；用 `cargo test session` 和 `cargo test agent` 验证两种接口、中断及旧记录兼容。

## 3. 桌面授权

- [x] 3.1 快照提供有效授权 ID，前端清理过期授权和已完成瞬态状态，入队清理旧授权；用 `make gui-test gui-check` 及 `cargo test gui_authorization` 验证重连和迟到事件隔离。

## 4. 文件路径及反馈

- [x] 4.1 文件工具识别 workspace 相对、本机绝对及包装引号路径；用 `cargo test tools` 验证两个 workspace 的同名文件、绝对路径、带空格与引号路径及不回退到其他目录。
- [x] 4.2 明确空文件、非文件和读取失败反馈，并确保完整结果仍遵守字节预算；用 `cargo test tools` 和 `cargo test --test tool_loop` 验证两种协议向模型传递正文及失败原因。

## 5. 综合验证

- [x] 5.1 顺序运行 `make check`，完成 `make gui-test gui-frontend`、GUI Rust 测试和 `make clippy-all`，最后执行 `openspec validate fix-review-session-consistency --strict`；记录实际结果和未实测平台。

## 验证结果（2026-09-30）

- `make check`：fmt、完整默认 Rust 测试（162 个单元测试及 44 个集成测试）、Clippy 全部通过，无新增警告。
- `cargo test --features gui --bin geer-agent`：164 个测试通过，覆盖 GUI 桥接代码。
- `make gui-test gui-check`：23 个前端测试及类型检查通过，包含过期授权清理、新授权保留、重连不丢新操作及恢复提交。
- `make clippy-all`：GUI 前端构建及带 gui feature 的 Rust Clippy 通过。
- `openspec validate fix-review-session-consistency --strict`、`git diff --check`：通过。
- SQL 事务与双协议持久化在本机 Linux + SQLite 实测；Windows 原生路径/硬链接和 PostgreSQL、MySQL、MongoDB 未实测。模型调用使用离线服务和注入 Provider，未请求真实模型接口。
- 修正可执行文件符号链接及 Bash 环境夹具后，验收与仓库配置隔离；清理了首次失败夹具误写的 9 条 mock 会话及其测试事件/Trace，保留其他会话和配置。
