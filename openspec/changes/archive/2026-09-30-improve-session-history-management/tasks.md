# Tasks

## 1. 展示标题与删除存储

- [x] 1.1 增加首条输入的内存投影和标题生成、持久化事件标题缓存，用 Rust 测试验证 Unicode、空会话、压缩及恢复且快照格式不变
- [x] 1.2 增加 SQL 事务删除与 MongoDB 分阶段删除，运行 DAO 契约测试验证幂等、Trace 保留、部分清理反馈及旧 revision 不复活
- [x] 1.3 完成 SessionManager 批量删除和当前会话替换，用测试验证待保存队列移除、内存/跨 workspace、失败保留与授权重置

## 2. 公共接口与命令

- [x] 2.1 扩展 Session 契约、删除预览/报告及 /delete 解析，运行命令与 Agent 测试确认完整 UUID、去重、错误和结构化结果
- [x] 2.2 接入 REPL 确认与管道 --yes，通过集成测试验证未确认不消费下一命令和删除后重启不复活

## 3. GUI 与 TUI

- [x] 3.1 扩展 GUI 命令结果与事件、管理模式和删除确认/重试，运行前端测试覆盖多选/全选、范围、草稿、忙碌和部分失败
- [x] 3.2 实现 TUI 会话管理/删除确认模式及标题显示，运行事件和渲染测试覆盖快捷键、范围、取消、草稿、结果与窄屏

## 4. 集成与质量门

- [x] 4.1 使用模拟双协议与临时数据库验证标题恢复和删除，完成本机 REPL、PTY/TUI 与 GUI 操作验收并记录外部数据库/Windows 验证边界
- [x] 4.2 运行 cargo fmt、默认及 GUI cargo test、前端 check/test/build、默认及 GUI Clippy、OpenSpec strict 和 git diff --check，更新验收记录
