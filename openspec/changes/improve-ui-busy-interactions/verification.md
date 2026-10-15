# 验收记录

## 范围与结论

本轮实现共用 GUI/Web 的运行中交互、草稿保护、流式呈现与 Mocha 视觉调整。原生 Rust Agent、命令协议和持久化未修改。交互分区与状态矩阵见 [design.md](design.md)，行为场景见 [gui delta](specs/gui/spec.md)。

## 已执行

| 检查 | 实际结果 |
| --- | --- |
| `make fmt` | 通过 |
| `make frontend-test` | 6 个文件、44 项通过；包括思考时编辑/选择、删除互斥、记忆关闭/Escape、迟到确认和跨会话草稿、历史渲染、滚动与增量顺序/卸载清理 |
| `./scripts/test-safe.sh make frontend-build` | GUI/Web 类型检查与两份构建通过，无依赖或锁文件变更 |
| `make test FEATURES=web TEST=ui::app` | 共用运行时 17 项通过，包括串行命令、授权、断开、重连快照与删除预览 |
| `make clippy` | 默认配置通过，无新增警告 |
| 受限 Chrome 模拟页面检查 | 1440×1000 桌面宿主和 375×812 Web 宿主均通过，无浏览器异常、无横向溢出 |

测试/浏览器检查全部经受限入口执行，服务实际预检通过：独立 tmpfs、有限内存/任务/运行时间、swap=0 与整组回收。测试服务未并发叠加。

## 浏览器场景

使用本地临时页面注入模拟 HostAdapter，120 轮含表格和代码块的历史，700ms 首段等待、5ms 一次的有界连续文本事件。检查实际输入、等待提示、记忆打开/关闭、会话范围和勾选、删除禁用、真实滚轮向上阅读、返回最新消息、继续输入和草稿保持。源代码与产物均不包含演示入口或数据，未调用真实模型、未读用户会话。

最后一次实际滚轮检查，桌面阅读位置保持在 46616px、手机保持在 54929px；返回最新后恢复跟随。连续输出时十次输入命令往返为桌面 15–82ms、手机 10–53ms；这是本机自动化往返观测，不是跨设备性能保证，也不是优化前后的基准对比。

截图和可重复的本地检查脚本位于忽略的 `target/ui-review/`（`desktop.png`、`mobile.png`、`results.json`、`check.mjs`）。清理构建目录会删除它们；模拟页面检查脚本通过 `./scripts/test-safe.sh node target/ui-review/check.mjs` 运行，退出后删除临时前端入口，服务回收浏览器及其后代。

初版检查脚本曾因字符串转义失败，随后修正；一次直接赋值 `scrollTop` 与自动跟随竞态导致检查失败，改用真实浏览器滚轮事件后两个尺寸均保持阅读位置。该失败未被记为产品测试通过，也未删除滚动位置断言。

## 综合检查

- `make check` 完整通过：fmt → 安全预检与 7 类回收探针 → 默认 Rust 测试 → clippy。Rust 共 262 项通过、1 项真实网络 smoke 按原规则忽略，无新增警告；不据此声称外部数据库或真实网络已验收。
- `openspec validate improve-ui-busy-interactions --strict` 通过。
- `python3 .ai/scripts/check_governance.py --repo . --framework` 通过：errors/warnings 均为 0，声明范围 125 个文件归属完整，unmapped/ambiguous/unchecked_symlinks 均为 0；检查器不验证锚点、业务语义或客户端规则加载。
- `git diff --check` 与新增文件尾随空白检查通过；已审查源码、测试、文档和新增文件。初始工作区干净，本轮未提交、推送、同步主 spec 或归档。

## 平台限制

本轮浏览器采用模拟宿主，验证共用 App 的交互与渲染，不代表原生 Tauri/WebView、真实 HTTP/WS 服务和真实模型长时间响应已完成端到端验收。Linux/Windows 原生窗口的剩余性能问题需在对应系统复核。发送、会话切换和数据修改仍等待当前操作完成；没有新增停止执行或自动排队。
