# Proposal

## Why

Windows 通用构建保留控制台能力，双击以 GUI 启动时会出现控制台；现有无控制台桌面构建需要另跑 Cargo，并覆盖同名通用产物。希望一次日常构建即可获得两个独立可执行文件。

## What Changes

- Windows 下 `make build` 默认同时生成 debug 通用程序 `geer-agent.exe` 和桌面程序 `geer-agent-desktop.exe`。
- Windows 下 `make release` 默认同时生成上述两个 release 程序，输出到对应 profile 目录，无需额外命令或参数。
- 桌面程序沿用现有只支持 GUI、默认进入 GUI、无额外控制台、启动错误在窗口显示的行为；通用程序继续通过环境配置选择四种界面。
- Linux 等非 Windows 环境的构建与 `make run` 保持现状。

## Capabilities

### New Capabilities

无。仅改变构建与产物交付入口，设置 `skip_specs: true`。

### Modified Capabilities

无。沿用已有通用与桌面构建的运行契约，不新增界面行为。

## Impact

修改 Make 构建入口、帮助信息、README、项目约定及架构文档中相关构建说明。不增加 Rust 或前端依赖，不改 Agent、工具或 GUI 的运行逻辑。Windows 构建耗时与缓存占用有所增加。

## Non-goals

不增加 `make debug`、独立桌面构建命令、安装器、交叉编译入口、动态控制台管理或新的 Rust 可执行入口；不改变默认 Cargo 构建或密钥打包行为。
