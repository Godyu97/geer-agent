# 工具注册与执行：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 工具准备、授权与执行

Tools 把模型工具调用映射到内建工具，先验证字段/路径，再决定是否需要确认；授权按工具分别记录，可关闭 tools。Bash、正文搜索、网页搜索和文件操作首次使用需授权；网页抓取每次确认完整 URL，跨来源重定向还会再次确认。相对文件路径按当前 workspace 解析，文件工具也接受绝对路径和 `~/`；同一工具获准后，本次会话可继续对其操作，不代表文件操作都限制在 workspace。正文 search 会规范化路径并拒绝 workspace 外位置。只读查询可并行，有副作用的操作串行。输出经 feedback 限长并携带成功、变化和元数据。

路径/授权边界的当前实现来源是 [tools 注册与准备](../../../src/tools/mod.rs)、[文件操作](../../../src/tools/file.rs) 和 [search_scope](../../../src/tools/query.rs)。[file-tools delta](../../../openspec/changes/file-tools/specs/file-tools/spec.md) 允许按工具获准后的绝对路径；[search-fetch delta](../../../openspec/changes/add-search-web-access/specs/search-fetch/spec.md) 只收紧正文 search，明确保留旧工具语义。两者尚不都在主 spec，不能把「默认不操作仓库外」治理约束写成已实现的统一沙箱，也不能据运行时工具授权扩大当前编码任务范围。

## 验证入口与缺口

代表性测试：[src/tools/tests.rs](../../../src/tools/tests.rs)、[Web 访问单测](../../../src/tools/web_access/tests.rs)、[tests/tool_loop.rs](../../../tests/tool_loop.rs)、[tests/process_safety.rs](../../../tests/process_safety.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

src/tools/tests.rs 覆盖授权、workspace、参数、批量执行和结果限长；tests/tool_loop.rs 验证 Agent 往返，tests/process_safety.rs 覆盖进程边界。本次未运行。

源代码入口：[src/tools](../../../src/tools)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
