# 开发工具与测试隔离：模块定义

## 快速定位

管理构建/验证命令、测试资源隔离、进程夹具和仓库内工作流入口；不承担 Agent 业务或 UI 渲染。

- 命令与清单：[Makefile](../../../Makefile)、[Cargo.toml](../../../Cargo.toml)、[OpenSpec 配置](../../../openspec/config.yaml)
- 安全入口：[test-safe.sh](../../../scripts/test-safe.sh)、[进程夹具](../../../tests/support/mod.rs)
- 代表性验证：[test-safe-check.py](../../../scripts/test-safe-check.py)、[process_safety.rs](../../../tests/process_safety.rs)
- 工作流：[.agents/skills](../../../.agents/skills)；路径归属见 [catalog](../../../.ai/catalog.json)

## 按需阅读

| 当前需要 | 读取位置 |
| --- | --- |
| 构建、受限服务、进程清理、生成来源 | [01 当前实现](01-implementation.md) |
| 规范状态或环境验证缺口 | [02 当前事项](02-issues.md) |
| 实际命令 / Git 与 OpenSpec 触发 | [项目手册](../../../.ai/01-project.md) / [工作流](../../../.ai/03-workflow.md) |

## 职责与边界

`build.rs` 的编译期资源处理由 [构建入口](../build-script/00-overview.md) 解释；本模块管理 Make 编排、根清单/锁文件/配置样例、scripts、tests/support 与已安装工作流。测试文件可由多个业务模块引用，进程安全实现只有一份。`.agents` 中的 skill 和 runner 是已有工具，不因归属登记而获得执行授权。
