# 25 自动化测试：验证可见行为

[返回总目录](../README.md) · [下一篇](02-cargo.md)

对应原教程：编写与执行测试、单元与集成测试、断言、GitHub Actions、benchmark。

## 三种测试各自靠近不同边界

单元测试跟模块放在一起，适合解析、预算和状态转换，也能通过 `super` 测私有函数。集成测试位于 `tests/`，通常从公开库接口或二进制入口验证跨模块行为。文档测试检查文档里的例子是否仍正确。

```rust
fn remaining(max: usize, used: usize) -> usize {
    max.saturating_sub(used)
}

#[test]
fn exhausted_budget_stays_zero() {
    assert_eq!(remaining(3, 5), 0);
}

fn main() {
    assert_eq!(remaining(3, 5), 0);
}
```

Cargo test 会执行带 test 属性的函数；本仓库和练习执行必须包在安全入口中。assert_eq 比只检查“没有崩溃”更具体；should_panic 适合验证确实约定要 panic 的接口，普通失败应检查 Result。

## 常用执行方式

```bash
make test
scripts/test-safe.sh cargo test parses_supported_commands
scripts/test-safe.sh cargo test --test responses_retry
scripts/test-safe.sh cargo test -- --nocapture
scripts/test-safe.sh cargo test -- --test-threads=1
```

安全入口还检查 cgroup 额度与私有临时目录，失败不得继续。最后一条的测试线程参数不证明程序本身没有并发，也不能独立替代隔离。--ignored 会显式运行忽略测试；先读其外部环境要求，不把它当默认检查步骤。

## 这份仓库的测试值得怎么读

- [interaction/tests.rs](../../../src/interaction/tests.rs) 与 [ui/repl/mod.rs](../../../src/ui/repl/mod.rs)：命令解析、非法 UTF-8 与确认输入是用户能观察到的行为；[command_tests.rs](../../../src/interaction/command_tests.rs) 检查共用执行中的流式顺序、错误传播与删除边界。
- [prompt/conversation.rs](../../../src/prompt/conversation.rs)：检查两种协议的历史、提交与重置。
- [tests/responses_retry.rs](../../../tests/responses_retry.rs)：本地 HTTP 服务检查重试次数与可见输出。
- [tests/tool_loop.rs](../../../tests/tool_loop.rs)：启动真实二进制，注入测试配置和模型响应，检查完整链路。
- [dao/mod.rs](../../../src/dao/mod.rs)：数据库读写契约，区分本地 SQLite 与需要配置的外部后端。

项目没有 lib crate，因此不能假设 `tests/` 可以直接导入所有 `pub(crate)` 项；现有集成测试通过 `CARGO_BIN_EXE_geer-agent` 找到构建出的二进制。

## 如何避免“通过但没测到重点”

测试失败路径、空值、上限边界和副作用顺序。例如批量读取不仅检查结果数量，还应检查输入顺序；重试不仅检查最终成功，还应检查到底发了几次请求和有没有重复正文。

网络与数据库尽量使用明确的测试资源。mock 通过证明的是受控协议场景，不能替代真实上游兼容性验证；数据库内存测试也不能自动证明所有后端事务行为相同。

## CI 与 benchmark 是后续工具

CI 将本地验证命令放到自动环境执行。本次笔记不创建 CI 配置；实际使用时需准备 feature 所依赖的文件/服务，并避免把本地密钥打包进去。

benchmark 测速度，应区分 debug/release、预热、I/O 波动和输入规模。标准库历史上的 `#[bench]` 路线涉及不稳定 test API；要在 stable 上用统计基准，先查看 Criterion 等工具的官方文档并按需要引入，而非直接复制旧版 nightly 配置。

来源：[教程测试](https://beatai.org/rust-course/test/intro)、[Cargo test](https://doc.rust-lang.org/cargo/commands/cargo-test.html)、[Rust Book 测试组织](https://doc.rust-lang.org/book/ch11-03-test-organization.html)、[Criterion](https://bheisler.github.io/criterion.rs/book/)。

进阶见 [行为、类型约束与属性验证](14-testing-and-contracts.md)，HTTP 实验见 [Axum 请求契约](../08-ecosystem/12-web-testing-and-middleware.md)。
