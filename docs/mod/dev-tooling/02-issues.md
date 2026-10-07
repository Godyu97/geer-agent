# 开发工具与测试隔离：当前事项

## 已实施能力与主 spec 覆盖（待核实同步）

主 spec 当前只有 gui、llm-trace、session-persistence、tool-loop、tui、workspace-management；下列能力已有代码和完成的 tasks，仍保留进行中 delta，尚未在主 spec 找到对应能力入口：

- [文件工具](../../../openspec/changes/file-tools/specs/file-tools/spec.md)：代码在 `src/tools/file.rs`，首次按工具授权并支持绝对路径。
- [项目指令与记忆](../../../openspec/changes/add-project-memory/specs/project-memory/spec.md)：根指令、记忆存储及四种界面已有实现。
- [主动记忆](../../../openspec/changes/add-active-memory-recall/specs/active-memory/spec.md)：Agent 本轮召回与 retrieval 已接入。
- [Web UI](../../../openspec/changes/add-web-ui)：共用 ui/app、认证/HTTP/WS 和 Web HostAdapter 已实现。

证据是当前源码路径、delta 与 tasks，不能据未归档状态宣称未实现，也不能凭 tasks 勾完宣称完整验收。下一步在用户要求相关 verify/sync/archive 时按既有 skill 核对实现与 delta，再处理规范状态；此轮不批量同步或归档，其他 change 按需再调查。

## 非 Linux 测试隔离（尚缺等效入口）

Makefile 对 Windows 的 test/clippy/check 明确失败；`test-safe.sh` 要求 Linux/systemd/cgroup v2。应用的 Windows 桌面支持不等于可在 Windows 安全运行现有测试。

下一步若任务要求目标平台测试，先提供内存、进程数量、总时限和后代/临时文件回收均有效的独立隔离环境；验证实际限额后再运行，不能降级裸测试。本轮没有为此修改系统或安装环境。

维护与状态规则见 [.ai/04-documentation.md](../../../.ai/04-documentation.md)。
