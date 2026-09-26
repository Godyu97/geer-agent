# 11 入门实战：做一个小型文本搜索器

[返回总目录](../README.md) · [上一篇](10-modules-and-docs.md) · [进入进阶](../02-advanced/01-lifetimes-and-static.md)

对应原教程：文件搜索工具的基本功能、模块化和错误、测试驱动、环境变量、stderr、迭代器改进。

## 把实践拆成六次可验证的改动

| 原教程步骤 | 本次应该学会的事 | 在 geer-agent 中对应 |
| --- | --- | --- |
| 基本功能 | 输入、读文件、筛选行、输出 | REPL 读取输入，文件工具读取文本 |
| 模块化和错误 | 入口协调，函数各管一件事 | `main` → `agent::run`，`config` 独立解析 |
| 测试驱动 | 先说清楚匹配规则 | `parse_input` 单元测试 |
| 环境变量 | 环境只在边界读取 | `Config::load` 与纯解析函数分开 |
| stderr | 正常结果与诊断分开 | 流式正文与错误/指标输出 |
| 迭代器改进 | 用管道表达筛选和收集 | `filter_map`、`map`、`collect` |

## 先写不碰文件的核心函数

```rust
fn search<'a>(query: &str, contents: &'a str) -> Vec<&'a str> {
    contents.lines().filter(|line| line.contains(query)).collect()
}

fn main() {
    let contents = "read Cargo.toml\nwrite note.md\nread src/main.rs";
    assert_eq!(search("read", contents), vec!["read Cargo.toml", "read src/main.rs"]);
    assert!(search("bash", contents).is_empty());
}
```

结果借用 `contents` 中的行，因此需要把输出生命周期关联到它；不关联到 `query`，因为结果并非来自查询字符串。没有文件系统和网络，这个函数很容易验证。

再由外层使用 `std::fs::read_to_string(path)?` 取得拥有型内容，在内容仍有效时打印匹配结果。读取错误向上传递；不要把读不到文件假装成“没有匹配行”。

## 测试驱动是在定义行为

先写三个验收条件：正常匹配、不匹配、大小写规则。再实现代码让它满足这些条件。如果要增加忽略大小写，明确当前练习只做什么范围的文本比较；简单 `to_lowercase` 不等于完整语言学意义上的大小写折叠。

测试环境配置时，优先给解析函数传入值，不通过改变整个进程的环境变量互相影响。edition 2024 中 `std::env::set_var` / `remove_var` 是 unsafe 接口；子进程测试可用 `Command::env` 设置新进程环境。[本项目集成测试](/home/lihongyu/projects/geer-agent/tests/tool_loop.rs) 就这样注入本地模拟服务地址。

## stdout 与 stderr 的价值

程序的匹配结果写 stdout，文件错误等写 stderr，调用者就能把结果保存到文件而不混入诊断。项目里同理：给用户看的模型正文与给开发者看的运行指标有不同用途。

## 给自己的练习

在独立练习工程中接收查询词和文件路径，调用 `search`，并让“文件不存在”返回非成功退出状态。验收时只用自己创建的临时文本，不扫描真实配置或密钥文件。进阶后再改用逐行缓冲读取，比较整文件读取与流式读取各自的内存占用和返回值所有权。

来源：[教程入门实战](https://beatai.org/rust-course/basic-practice/intro)、[Rust Book CLI 实战](https://doc.rust-lang.org/book/ch12-00-an-io-project.html)、[edition 2024 环境变量接口](https://doc.rust-lang.org/edition-guide/rust-2024/newly-unsafe-functions.html)。
