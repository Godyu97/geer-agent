# Proposal

## Why

当前 `make build` 是 debug 构建，而 `make gui-build` / `make web-build` 是 release 构建，`make gui` / `make web` 则会启动程序。命令名称没有一致表达构建模式和启动行为，需要整理成初学者能够直接对照 Cargo 的入口，同时将 Web 默认端口调整为用户指定的 8827。

## What Changes

- Web 未配置端口时监听 `0.0.0.0:8827`；有效的 `GEER_AGENT_WEB_PORT` 仍覆盖默认值。
- 推荐入口统一为 make build / make release / make run，默认包含 GUI、Web、TUI 和 REPL；启动时通过 GEER_AGENT_UI 选择界面，进程环境优先于 .env。build 只构建 debug，release 只构建 release，run 构建并启动 debug，通用命令不覆盖 UI 选择。
- Make 的三个通用入口固定包含 gui,web；精简构建直接使用 Cargo。test/clippy 默认仍检查终端配置，显式 FEATURES 控制受检功能。
- **BREAKING**：按用户要求移除旧 gui/web/gui-build/web-build、GUI 兼容别名和 embed 便捷入口，不维护两套构建约定。仅构建静态前端使用 frontend-build，完整程序使用 build/release/run。
- 帮助和文档列出 Cargo 等价调用、feature、产物目录和 ENV 启动示例，区分静态前端与完整程序构建。

## Capabilities

### New Capabilities

- `web-ui`：沿用已有 `add-web-ui` change 的能力路径，并非引入第二套浏览器能力。该能力尚未归档到主 specs，本次补充默认端口为 8827 及通用程序按 ENV 选择入口的契约。

### Modified Capabilities

无已归档能力需要修改。Makefile 整理属于开发工具，不另建工具链能力 spec。

## Non-goals

不新增 UI 类型或 Windows 启动器、不调整 Windows 控制台连接、不增加 release 运行命令、不调整测试安全限制、不添加依赖、不改模型、认证或会话逻辑。本变更不对应新的教程日程能力。Windows 通用程序保留控制台能力；双击完全无控制台的 desktop-gui 专用能力仍可通过 Cargo 构建，本次不改变它的运行限制。

## Impact

涉及 `src/config/web.rs` 及其测试、`Makefile`、`.env.example`、`README.md`、`AGENTS.md` 的命令说明和 `docs/design/architecture.md` 的端口说明。需要同步尚未归档的 `add-web-ui` 规划中明确写出的旧默认端口，避免两份当前规划互相矛盾；历史归档和任意测试端口不做全仓替换。无新依赖和数据迁移。
