# Proposal

## Why

GUI 使用 Responses 接口时，流式正文在请求结束后消失，恢复的历史记录也缺少回答。同轮工具调用逐条占用消息区，现有称呼和配色也需要按用户要求调整。

## What Changes

- 流式回答结束和恢复历史后仍显示完整正文，包括工具调用前后的模型说明。
- 用户称呼统一为「李火旺🔥」，助手称呼统一为「Geer」。
- 桌面界面采用 Catppuccin Mocha 配色。
- 每次用户提问中的工具调用合并到一个默认折叠的区块，展开查看详情；正文保持可见。

## Capabilities

### New Capabilities

- `gui`：沿用 `add-desktop-gui` 已定义但尚未归档的能力路径，补充正文保留、称呼、配色和按轮折叠要求；不重复建立 GUI 基线。

### Modified Capabilities

无已归档能力需要修改。

## Impact

涉及展示历史投影、GUI 事件桥接、React 消息列表与样式，以及对应回归验证。不新增依赖，不迁移数据库；原有 Responses 原始记录中的正文可直接恢复。归档时应先处理 `add-desktop-gui` 基线。

## Non-goals

不改变模型请求、工具执行或授权规则；不增加主题切换器、不重构终端界面、不承诺本次完成原 GUI 变更中的全部跨平台真机验收。
