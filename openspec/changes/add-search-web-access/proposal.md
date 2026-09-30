# Proposal

## Why

对应 GeekAgent Day12 和能力 `search-fetch`。现有正文查询允许任意路径，模型需要一个明确限制在当前 workspace 内、命中后可直接继续读取的搜索入口，同时需要发现网页及读取网页正文的能力。

## What Changes

- 新增 `search`：支持正则、字面匹配、大小写与路径过滤，返回相对 workspace 的命中行或文件列表，明确区分失败、无命中和截断。
- 新增 `web_search`：免 Key 搜索网页，返回标题、完整 URL、摘要和搜索后端。
- 新增 `web_fetch`：确认后读取公网、本地或内网页面及文本，跨来源重定向再次确认，返回来源与可读内容。
- 新工具沿用现有启停、会话授权、工具历史、用量和界面展示；已有工具保持原行为。

## Capabilities

### New Capabilities

- `search-fetch`：受 workspace 限制的本地搜索、网页搜索及逐次确认的网页抓取。

### Modified Capabilities

无。继续遵守现有 `tool-loop` 的调度、结果配对和启停契约。

## Impact

扩展工具注册与查询模块，增加网页访问模块；更新系统提示、README 和架构导读。直接使用已有传递依赖 reqwest，新增 html2text 与 encoding_rs，分别负责 HTML 转文本及有限读取后的字符集解码。测试复用受限服务和本地模拟服务器。

## Non-goals

不改变已有文件工具的目录权限，不增加 PDF、JavaScript 渲染、浏览器、缓存、托管抓取回退、多搜索后端、通用 MCP 接入或新 UI 框架。
