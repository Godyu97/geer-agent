# 构建入口：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## feature 与静态资源

启用 web 时，构建脚本追踪 dist/web 并要求 index.html 已存在；启用 gui 时追踪 dist/gui，并用 Tauri 配置与 capabilities 文件运行 tauri_build。资源缺失会提示先运行 make frontend-build，构建失败直接中止。默认 cargo build 不启用图形 feature，因此不要求静态前端。

`build.rs` 负责追踪/预检，Web 静态文件实际由 [ui/web](../../../src/ui/web/mod.rs) 的 include_dir 嵌入；GUI 资源由 [Tauri 配置](../../../src/ui/gui/tauri.conf.json) 指向 `dist/gui`。前端生成来源与 Windows 桌面输出隔离见 [工程边界](../../../.ai/02-engineering.md#生成与维护边界)，不直接修改 dist。

## 验证入口与缺口

构建脚本随 `cargo build` 执行，没有独立 build.rs 单元测试；图形路径需先 `make frontend-build`，再按目标 feature 构建。运行目录、平台条件与命令见 [项目手册](../../../.ai/01-project.md)。本轮只静态核对 feature 分支与资源消费路径，未运行默认或图形构建，不据此登记一个必须补单元测试的缺陷。

源代码入口：[build.rs](../../../build.rs)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
