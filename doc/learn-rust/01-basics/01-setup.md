# 01 环境、Cargo 与第一次运行

[返回总目录](../README.md) · [下一篇](02-values-and-functions.md)

对应原教程：关于本书、进入 Rust 编程世界、避免从入门到放弃，以及“寻找牛刀，以便小试”的全部小节。

## 先建立三个工具的关系

| 工具 | 负责什么 | 常用命令 |
| --- | --- | --- |
| `rustup` | 安装和选择 Rust 工具链 | `rustup show` |
| `rustc` | 将 Rust 源码编译成目标代码 | `rustc --version` |
| `cargo` | 管依赖、构建、运行和测试 | `cargo check` |

先执行版本命令确认环境。缺少环境时按 [Rust 官方安装页](https://www.rust-lang.org/tools/install) 操作；Windows 还要区分 MSVC/GNU 目标所需的链接器。安装 Rust 标准库目标不等于装好了交叉链接工具。

编辑器提供语法提示，`rust-analyzer` 提供跳转、类型提示和诊断；最终以实际编译结果为准。不需要先装一堆插件，也不要为读教程改动项目工具链。

## 第一个独立示例

```rust
fn main() {
    let name = "Rust 初学者";
    println!("你好，{name}");
}
```

可以在仓库之外创建练习目录，然后将代码放入它的 `src/main.rs`：

```bash
cargo new rust-playground
cd rust-playground
cargo run
```

不要用这些步骤覆盖 geer-agent 的入口。`fn main` 是入口函数，`println!` 是宏，字符串里的 `{name}` 会读取当前变量。

## Cargo 命令的区别

```bash
cargo check
cargo build
cargo run
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets --all-features
```

`check` 检查代码而省去通常的最终机器码生成/链接，适合频繁迭代；`build` 生成可执行文件；`run` 先构建再执行；`test` 构建并运行测试。格式检查不保证逻辑正确，Clippy 也不能替代测试。

`Cargo.toml` 写项目和依赖要求，`Cargo.lock` 记录解析后的依赖版本。应用项目应保留 lock 文件；不要因为看不懂就删掉重新下载。

## 在本项目中读什么

打开 [Cargo.toml](/home/lihongyu/projects/geer-agent/Cargo.toml) 和 [main.rs](/home/lihongyu/projects/geer-agent/src/main.rs)。入口很短：声明模块，建立 Tokio 运行时，再调用 `agent::run().await`。暂时只记住“初始化运行环境后启动业务”，异步细节后面再读。

`cargo run` 会进入真实应用，需要配置 `OPENAI_API_KEY` 和 `OPENAI_MODEL`；模型地址、协议等由 [Config::load](/home/lihongyu/projects/geer-agent/src/config/mod.rs) 解析。学习语法和运行笔记里的标准库例子不需要 API key。

本仓库 `--all-features` 会启用 `embed-env`，它在编译时读取 `.env`。缺少文件时构建会失败；生成的程序也可能包含配置明文。这个 feature 的具体边界见 [Cargo 工程篇](../03-engineering/02-cargo.md)。

## 下载慢时先辨别问题

下载慢可能来自网络、代理或 registry；提示等待 package cache 锁，可能只是另一个 Cargo 进程正在工作。先看错误和活动进程，再查 [Cargo 网络配置](https://doc.rust-lang.org/cargo/reference/config.html#net)。`--offline` 只能使用已缓存依赖，不能修复缺包。教程中的镜像地址可能随时间变化，不直接复制历史地址作为通用配置。

## 学完后自测

为什么 `cargo check` 通过后仍可能在运行时失败？因为环境变量、网络、文件和业务输入并不由类型检查保证。修改示例中的问候语并运行一次，比背下所有命令更有帮助。

来源：[教程 Cargo 小节](https://beatai.org/rust-course/first-try/cargo)、[官方 Cargo 入门](https://doc.rust-lang.org/cargo/getting-started/first-steps.html)。
