# 长期记忆：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 记忆读写与召回

MemoryService 从所选存储加载记录，添加时按正文去重，支持编辑、单项删除、清空和管理搜索；搜索对大小写不敏感并按命中排序、限制结果数。Agent 可在请求前主动召回相关内容：retrieval 对记忆分块和排序，memory 再按 token 预算选择片段。数据库失败报告 unavailable/error，不将缓存列表回写成“成功”。

## 验证入口与缺口

代表性测试：[src/memory/tests.rs](../../../src/memory/tests.rs)、[src/tools/tests.rs](../../../src/tools/tests.rs)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

src/memory/tests.rs 覆盖持久化、去重、搜索、失败和召回；src/tools/tests.rs 覆盖记忆工具。本次未运行。

源代码入口：[src/memory](../../../src/memory)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
