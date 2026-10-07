# 桌面 GUI 宿主：当前事项

仅保存当前相关的未关闭决定、风险与验证缺口，不记录日常命令流水。状态变更需要实际证据；文档整理不等于验收。

## 当前清单

### 原生窗口与 Windows 桌面产物（待目标平台验收）

[构建命令变更的验收记录](../../../openspec/changes/clarify-ui-build-commands/tasks.md#验证记录) 明确宿主为 Linux，未执行 Windows 原生或 GUI 窗口交互验收。[桌面启动 tasks](../../../openspec/changes/improve-gui-launch/tasks.md) 包含交叉检查和模式回归，但这些证据不能证明 Windows 桌面程序的窗口、目录选择、剪贴板和关闭保存均已实机验收。

本轮核对了 GUI bridge、桌面模式选择和 Make 的桌面产物分离，没有运行平台构建或窗口交互。下一步在相关 GUI/Windows 任务中按 README 构建目标产物，验收 auto/gui 选择、启动错误可见、原生能力、授权及关闭保存，并记录平台/产物与未覆盖步骤。

维护规则与归档条件见 [.ai/04-documentation.md](../../../.ai/04-documentation.md)。稳定职责进入 00，已落地规则进入 01；尚未实现的目标留在对应需求或变更记录。
