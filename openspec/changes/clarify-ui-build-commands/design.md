# Design

## Context

动机见 proposal.md。ui::run 已支持通过 GEER_AGENT_UI 分派，gui,web 可同时编入同一程序；终端能力始终存在。desktop-gui 是限制只使用 GUI 的独立窗口子系统 feature。GUI/Web 前端共用一个包，静态资源分别位于 dist/gui 和 dist/web。

## Goals / Non-Goals

**Goals:** 用最少入口构建通用程序，启动时由 ENV 选择界面，清楚区分 debug、release 与运行。

**Non-Goals:** 不维护旧 Make 兼容目标，不增加平台 API、启动器、依赖或通用参数框架，不改变 Cargo 默认 feature 和 desktop-gui 的运行契约。

## Decisions

### 1. 固定通用构建，无条件变量组合

| 命令 | 对应 Cargo | 含义 |
| --- | --- | --- |
| make build | cargo build --features gui,web | 仅生成 target/debug/geer-agent |
| make release | cargo build --release --features gui,web | 仅生成 target/release/geer-agent |
| make run | cargo run --features gui,web | debug 构建并按 ENV 启动 |

Windows 产物带 .exe。三个目标固定依赖 frontend-build，后者冻结安装后顺序构建 GUI/Web 静态资源。不要增加 Make profile/UI/feature 的组合判断，精简编译直接使用 Cargo。FEATURES 仅保留给 test/clippy/doc，默认为空；GUI/Web 测试准备资源通过 frontend-build。

移除 gui/web/gui-build/web-build/gui-deps/gui-check/gui-test/gui-frontend/web-frontend/embed 等旧入口。保留有实际用途的 frontend-deps/frontend-build/frontend-check/frontend-test 与质量检查命令。embed-env 或 desktop-gui 的特殊功能通过明确 Cargo 命令使用，不为其维护额外 Make 别名。

### 2. 启动按 ENV 分派

通用入口不 export GEER_AGENT_UI。沿用进程环境优先于 .env 的加载顺序；gui/web/tui/repl 指定实际 UI，auto 按交互终端选择 TUI、按管道选择 REPL。每次启动选一个宿主，不同时启动多个 UI。精简程序未包含所选 UI、没有交互终端却选择 TUI 时仍明确失败。修改缺失 UI 的报错以指向现存 make build，更新相应回归。

现有所有权、Session、工作线程、错误处理和异步/流式路径不变。教程配置语义映射到已有 Rust 配置模块，不添加 crate，不重写业务入口。

### 3. Windows 通用程序保留控制台能力

通用构建启用 gui,web，不启用 desktop-gui，支持终端输入输出与管道。GUI 可从已有终端运行；双击可能有控制台。完全无控制台的专用桌面能力仍可在 frontend-build 后用 cargo build --release --features desktop-gui 构建，它仍只支持 GUI。本次不实现动态 AttachConsole 或新启动器，避免扩大平台范围。

### 4. Web 默认端口与文档

只调整 WebConfig 缺省分支和已有断言为 8827，保留有效覆盖、范围校验和错误报告。同步 .env.example、help、README、AGENTS 与架构说明，活动 add-web-ui 规划同步默认值和新构建约定；任意测试端口与历史归档不全仓替换。

## Risks / Trade-offs

- [Make 通用构建需要 Bun 与 GUI 原生依赖] → 文档列出依赖，轻量入口保持 cargo build/run。
- [Cargo 特殊 feature 构建更新同名产物] → 文档说明切回通用程序需重新 make build/release。
- [Windows 双击通用 GUI 可有控制台] → 清楚区分通用和 desktop-gui，保留终端兼容性。
- [测试隔离不可用] → 报告阻塞，不绕过安全入口。

## Migration Plan

端口单项回归后，精简 Make、补充全界面 ENV 覆盖回归并更新说明；用 make -n 验证 build/release/run 与检查入口；经受限服务验证 debug/release 构建、相关测试、clippy-all，最后 make check。无数据迁移。
