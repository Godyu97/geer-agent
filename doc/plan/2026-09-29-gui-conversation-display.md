# GUI 正文修复与对话展示优化

关联变更：`openspec/changes/fix-gui-conversation-display/`。本方案先于实现落盘。

## 已定位问题

`src/provider/openai/responses.rs` 保留 Responses 的结构化 `output`，但 `ModelStep.text` 留空。`src/prompt/conversation.rs` 的展示历史只读取原始事件的 `text`。流式结束时前端清空临时正文并采用历史快照，因此出现回答消失；保存后的原始 `output` 仍在，无需改数据库。

## 实施顺序

1. 修复展示投影：非空文本优先，空文本从公开的助手输出读取；同时覆盖工具轮次和恢复历史。先写能重现问题的 Rust 回归测试。
2. GUI 工具进度单独传递。前端以一次用户提问为一轮，同轮工具放入单个默认折叠区块；整轮加载更早历史，避免分页拆组。正文始终在折叠区外。
3. 用户显示名及输入身份使用「李火旺🔥」，助手和提示使用「Geer」。颜色使用 Catppuccin Mocha CSS 变量，保持现有布局并修正较长用户名空间。
4. 运行 Rust 格式化、相关测试、Clippy；运行前端测试、类型检查和构建；检查 OpenSpec 与 diff。用浏览器检查正常回答、工具折叠、主题和窄窗口布局，记录平台限制。

## 范围与验证边界

保留 Prompt / Session / Provider 职责，不增加依赖，不改变工具权限、模型请求和数据库内容。不访问真实会话正文或模型密钥；回归与界面验收使用合成数据。浏览器模拟 GUI 事件只能验证前端，不能视为真实模型或 Windows 原生窗口已验证。

## 执行结果

2026-09-29 已完成：

- 展示层从 Responses 的公开消息输出恢复正文，覆盖工具前说明、最终回答、序列化后恢复和去重。新增回归测试在修复前重现「仅剩用户消息与工具结果」，修复后通过。
- GUI 工具进度使用独立事件；同轮工具默认折叠，展开显示全部结果，长结果在块内滚动。正文保持在块外，历史按完整轮次加载。实时正文跨工具步骤保留段落间隔。
- GUI 用户消息和输入身份显示「李火旺🔥」，助手及提示显示「Geer」；界面统一使用 Catppuccin Mocha，并适配窄窗口的角色标签和侧栏标识。

验证结果：

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all`、`cargo fmt --all -- --check` | 通过 |
| `cargo test` | 167 项通过（129 单元 + 38 集成）；模型使用模拟接口 |
| `cargo clippy --all-targets` | 通过，无新增警告 |
| `cargo clippy --target x86_64-pc-windows-gnu --features gui --all-targets` | 通过，覆盖 GUI Rust 与测试代码的交叉编译检查 |
| 前端 `npm test` | 15 项通过，含完成后正文保留、实时工具隔离、跨轮归组、展开/收起、会话切换及整轮分页 |
| 前端 `npm run build` | 类型检查及静态产物构建通过 |
| `openspec validate fix-gui-conversation-display --strict`、`openspec validate --specs --strict` | 通过 |
| `git diff --check` | 通过 |

独立 Chrome 使用合成的 GUI 事件检查了 1160×760 和 720×520 布局：正文完成后仍可见、单轮一个默认折叠区块、长工具结果内部滚动、回车收起、代码复制、实时工具计数和 Mocha 计算色值均通过；未出现页面异常或横向溢出。默认折叠、展开和窄窗口截图已作视觉检查。

平台边界：内置 Browser 因插件运行时模块缺失而无法启动，改用隔离的本地 Chrome。`pkg-config` 未找到 `webkit2gtk-4.1` 和 `gtk+-3.0` 开发依赖，因此未运行本机原生 Tauri 窗口；Windows 结果为交叉编译检查，不代表 Windows 11 窗口实测。本次没有调用真实模型服务，也没有修改 GUI 基线 change 的跨平台验收状态。
