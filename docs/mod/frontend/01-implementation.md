# 共用 React 前端：当前实现

阅读路由：实现问题先检索下面的业务 / 行为标题，再核对代码；验证问题看「验证入口与缺口」。不把未实现需求写成本文件的当前行为。

## 事件状态与宿主适配

App 通过 HostAdapter 建立连接、提交命令、回复授权、复制文本和关闭；protocol.ts 约束 Rust 事件与快照类型，model.ts 的 reducer 把事件转换为 UI 状态，App.tsx 呈现对话、会话、记忆和确认流程。Vite 的 gui/web 模式把 @host 映射到对应实现并分别输出 dist/gui、dist/web；WebGate 管理浏览器登录。草稿按会话 ID 暂存，断线/快照 revision 与确认预览由 reducer 校验。

`protocol.ts` 是人工维护的 Rust JSON 消费类型，Rust 序列化定义在 [ui/app/events.rs](../../../src/ui/app/events.rs) 及其引用类型，双方没有自动 schema 生成。新增状态/字段时同步生产端、协议类型、reducer 和宿主调用，不能把 TypeScript 类型检查当作跨语言契约已一致的证明。Web 登录口令只用于认证，不把模型密钥或服务端配置放进浏览器构建变量。

## 运行期间的交互

会话写操作仍串行：发送、新建/打开、workspace 切换、删除、保存、压缩和记忆写入等待当前操作结束。本地会话范围、管理勾选和聊天草稿可在模型运行中编辑；草稿不会自动发送，提交成功回包只清除该次提交后未再编辑的输入。记忆面板在忙碌或断线时仍可搜索和关闭，Escape 先退出编辑器再关闭面板。

`stream.ts` 对同一请求的连续正文做 40ms 合并，工具、授权、快照等边界先刷新正文再立即分发，断开时丢弃尚未送出的缓存并清理计时器。历史轮次和消息 memo，复制回调保持稳定，活动正文使用延迟值，避免输入时重解析长历史。输入区显示真实可观察的运行状态；向上阅读暂停跟随，点击“返回最新消息”恢复。

## 验证入口与缺口

代表性测试：[App.test.tsx](../../../src/ui/frontend/src/App.test.tsx)、[model.test.ts](../../../src/ui/frontend/src/model.test.ts)、[Web.test.tsx](../../../src/ui/frontend/src/Web.test.tsx)、[MemoryPanel.test.tsx](../../../src/ui/frontend/src/MemoryPanel.test.tsx)、[markdown.test.ts](../../../src/ui/frontend/src/markdown.test.ts)、[stream.test.ts](../../../src/ui/frontend/src/stream.test.ts)。运行目录、命令及环境条件见 [项目手册](../../../.ai/01-project.md)。

src/ui/frontend/src 下的 Vitest 覆盖 reducer、Markdown、App、Web、记忆面板和流式调度；make frontend-check 做类型检查，make frontend-test 经受限入口运行。运行期间交互优化的验证范围与平台限制见 [验收记录](../../../openspec/changes/improve-ui-busy-interactions/verification.md)。

源代码入口：[src/ui/frontend](../../../src/ui/frontend)。存在测试文件不代表本次已运行或覆盖所有入口；已知未完成项见 [02](02-issues.md)。
