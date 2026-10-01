# Tasks

## 1. 桌面构建与启动

- [x] 1.1 增加桌面构建特性、Windows 子系统与默认 GUI 模式；以受限模式选择测试和 Windows 交叉检查验证，补充双击验收步骤。
- [x] 1.2 环境加载与模式错误复用 GUI 启动错误通道；受限运行 GUI Rust 回归并记录配置无效、缺 key 和误选终端的人工验收步骤。

## 2. 后台进程

- [x] 2.1 统一生产后台命令的 Windows 无窗口标志，覆盖版本探测、工具执行及超时清理；先 make test-safety，再受限缺失命令单项和 process_safety、Bash 回归验证。

## 3. 交付与检查

- [x] 3.1 增加 make gui-build、Linux 桌面启动器模板、README 和 doc/plan 原因及方案总结；以 make -n gui-build、桌面模板检查与文档复核验证。
- [x] 3.2 顺序执行 make check、受限 GUI feature 测试及 clippy、Windows 桌面构建交叉检查和 OpenSpec 严格校验；记录结果与真机验收限制。
