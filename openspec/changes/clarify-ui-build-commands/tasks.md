# Tasks

## 1. Web 默认值

- [x] 1.1 调整 Web 缺省端口和断言为 8827，执行 make fmt，再经 make test TEST=defaults_fixed_token_and_port_validation 验证默认、覆盖和非法配置，核对实际隔离有效。

## 2. 通用构建与 ENV 启动

- [x] 2.1 将 build/release/run 固定为 gui,web 并准备共用前端，用 frontend-build 代替旧 GUI/Web 专用构建入口；通过 make -n 检查 profile、前端构建和 ARGS，确认不覆盖 GEER_AGENT_UI、test/clippy 默认仍是轻量配置。
- [x] 2.2 精简 help，推荐 build/release/run，说明 Cargo 等价调用、产物目录、ENV 选择和 Cargo 精简入口；运行 make help 并检查旧兼容目标已删除。
- [x] 2.3 扩展 ENV 选择回归使其支持 gui,web，验证配置文件 GUI 可被进程 ENV 覆盖为 REPL、auto 与四种显式模式；更新缺失 UI 提示指向现有命令，受限执行相关单项。

## 3. 说明与验收

- [x] 3.1 更新 README、.env.example、AGENTS 和架构说明，写明通用构建、ENV 示例、Cargo 精简与特殊桌面入口及 8827；同步 add-web-ui 活动规划，用 rg 和 openspec validate --strict 验证一致。
- [x] 3.2 顺序受限执行 make build 与 make release，确认通用产物生成；执行 make fmt、相关全界面 Rust 回归、make clippy-all 与 make check，核对实际隔离有效，报告当前宿主无法验证的 Windows/原生窗口部分。

## 验证记录

- 全界面 debug 与 release 构建通过；release 首次冷编译触发默认 10 分钟时限，确认组已回收后，以有限 15 分钟时限重试通过，内存 4G、无 swap、任务 256 和私有 tmpfs 均保持有效。
- make test FEATURES=gui,web：247 项通过，1 项真实网络手动测试按原约定忽略。
- make frontend-test：30 项通过。make clippy-all 和 make check 通过，无新 clippy 警告；make check 包含 239 项默认 Rust 回归及全部隔离/异常清理探针。
- 当前宿主为 Linux，未执行 Windows 原生或 GUI 窗口交互验收；通用 ENV 分派与 REPL 覆盖由组合功能测试验证。
