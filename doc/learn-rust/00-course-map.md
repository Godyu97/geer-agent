# 原教程逐章对照

[返回学习入口](README.md)

依据作者仓库提交 `ebe2d82437f6` 的 [SUMMARY.md](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/SUMMARY.md)，按原目录顺序列出 **300 个 Markdown 条目**。条目包括目录导读、正文、历史版本和占位页，不等于 300 篇完整长文。原书 HTML 注释中的隐藏规划不纳入本表；附录的无路径分组不是一个内容页。

点击原章名查看本次使用的固定版本源码；原站入口保留在各篇来源中。相近小节合并到一篇笔记，基础内容详细讲解，Web/Redis/链表以逐节实践导读覆盖。

状态说明：“正文”表示存在主题内容，不保证原文所有示例与当前工具链一致；“目录/导读”主要组织后续阅读；“TODO/doing，已有内容”保留原目录标记；“占位/外链”表示该快照没有成篇讲解，本地对应内容是补充。

## 前言与学习方法

| 序号 | 教程条目（固定源码） | 本地笔记 | 原文状态 |
| --- | --- | --- | --- |
| 001 | [关于本书](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/about-book.md) | [01 环境、Cargo 与第一次运行](01-basics/01-setup.md) | 正文 |
| 002 | [进入 Rust 编程世界](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/into-rust.md) | [01 环境、Cargo 与第一次运行](01-basics/01-setup.md) | 正文 |
| 003 | [避免从入门到放弃](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/first-try/sth-you-should-not-do.md) | [01 环境、Cargo 与第一次运行](01-basics/01-setup.md) | 正文 |

## Rust 语言基础学习

| 序号 | 教程条目（固定源码） | 本地笔记 | 原文状态 |
| --- | --- | --- | --- |
| 004 | [寻找牛刀，以便小试](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/first-try/intro.md) | [01 环境、Cargo 与第一次运行](01-basics/01-setup.md) | 目录/导读 |
| 005 | [· 安装 Rust 环境](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/first-try/installation.md) | [01 环境、Cargo 与第一次运行](01-basics/01-setup.md) | 正文 |
| 006 | [· 墙推 VSCode!](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/first-try/editor.md) | [01 环境、Cargo 与第一次运行](01-basics/01-setup.md) | 正文 |
| 007 | [· 认识 Cargo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/first-try/cargo.md) | [01 环境、Cargo 与第一次运行](01-basics/01-setup.md) | 正文 |
| 008 | [· 不仅仅是 Hello world](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/first-try/hello-world.md) | [01 环境、Cargo 与第一次运行](01-basics/01-setup.md) | 正文 |
| 009 | [· 下载依赖太慢了？](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/first-try/slowly-downloading.md) | [01 环境、Cargo 与第一次运行](01-basics/01-setup.md) | 正文 |
| 010 | [Rust 基础入门](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/intro.md) | [01 环境、Cargo 与第一次运行](01-basics/01-setup.md) | 目录/导读 |
| 011 | [· 变量绑定与解构](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/variable.md) | [02 变量、基本类型、表达式与函数](01-basics/02-values-and-functions.md) | 正文 |
| 012 | [· 基本类型](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/base-type/index.md) | [02 变量、基本类型、表达式与函数](01-basics/02-values-and-functions.md) | 目录/导读 |
| 013 | [· · 数值类型](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/base-type/numbers.md) | [02 变量、基本类型、表达式与函数](01-basics/02-values-and-functions.md) | 正文 |
| 014 | [· · 字符、布尔、单元类型](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/base-type/char-bool.md) | [02 变量、基本类型、表达式与函数](01-basics/02-values-and-functions.md) | 正文 |
| 015 | [· · 语句与表达式](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/base-type/statement-expression.md) | [02 变量、基本类型、表达式与函数](01-basics/02-values-and-functions.md) | 正文 |
| 016 | [· · 函数](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/base-type/function.md) | [02 变量、基本类型、表达式与函数](01-basics/02-values-and-functions.md) | 正文 |
| 017 | [· 所有权和借用](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/ownership/index.md) | [03 所有权、移动、借用与复制](01-basics/03-ownership.md) | 目录/导读 |
| 018 | [· · 所有权](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/ownership/ownership.md) | [03 所有权、移动、借用与复制](01-basics/03-ownership.md) | 正文 |
| 019 | [· · 引用与借用](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/ownership/borrowing.md) | [03 所有权、移动、借用与复制](01-basics/03-ownership.md) | 正文 |
| 020 | [· 复合类型](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/compound-type/intro.md) | [04 字符串、切片、元组、结构体、枚举与数组](01-basics/04-strings-and-compound-types.md) | 目录/导读 |
| 021 | [· · 字符串与切片](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/compound-type/string-slice.md) | [04 字符串、切片、元组、结构体、枚举与数组](01-basics/04-strings-and-compound-types.md) | 正文 |
| 022 | [· · 元组](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/compound-type/tuple.md) | [04 字符串、切片、元组、结构体、枚举与数组](01-basics/04-strings-and-compound-types.md) | 正文 |
| 023 | [· · 结构体](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/compound-type/struct.md) | [04 字符串、切片、元组、结构体、枚举与数组](01-basics/04-strings-and-compound-types.md) | 正文 |
| 024 | [· · 枚举](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/compound-type/enum.md) | [04 字符串、切片、元组、结构体、枚举与数组](01-basics/04-strings-and-compound-types.md) | 正文 |
| 025 | [· · 数组](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/compound-type/array.md) | [04 字符串、切片、元组、结构体、枚举与数组](01-basics/04-strings-and-compound-types.md) | 正文 |
| 026 | [· 流程控制](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/flow-control.md) | [05 流程控制与模式匹配](01-basics/05-control-flow-and-patterns.md) | 正文 |
| 027 | [· 模式匹配](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/match-pattern/intro.md) | [05 流程控制与模式匹配](01-basics/05-control-flow-and-patterns.md) | 目录/导读 |
| 028 | [· · match 和 if let](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/match-pattern/match-if-let.md) | [05 流程控制与模式匹配](01-basics/05-control-flow-and-patterns.md) | 正文 |
| 029 | [· · 解构 Option](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/match-pattern/option.md) | [05 流程控制与模式匹配](01-basics/05-control-flow-and-patterns.md) | 正文 |
| 030 | [· · 模式适用场景](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/match-pattern/pattern-match.md) | [05 流程控制与模式匹配](01-basics/05-control-flow-and-patterns.md) | 正文 |
| 031 | [· · 全模式列表](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/match-pattern/all-patterns.md) | [05 流程控制与模式匹配](01-basics/05-control-flow-and-patterns.md) | 正文 |
| 032 | [· 方法 Method](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/method.md) | [06 方法、泛型、Trait 与特征对象](01-basics/06-methods-and-traits.md) | 正文 |
| 033 | [· 泛型和特征](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/trait/intro.md) | [06 方法、泛型、Trait 与特征对象](01-basics/06-methods-and-traits.md) | 目录/导读 |
| 034 | [· · 泛型 Generics](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/trait/generic.md) | [06 方法、泛型、Trait 与特征对象](01-basics/06-methods-and-traits.md) | 正文 |
| 035 | [· · 特征 Trait](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/trait/trait.md) | [06 方法、泛型、Trait 与特征对象](01-basics/06-methods-and-traits.md) | 正文 |
| 036 | [· · 特征对象](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/trait/trait-object.md) | [06 方法、泛型、Trait 与特征对象](01-basics/06-methods-and-traits.md) | 正文 |
| 037 | [· · 进一步深入特征](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/trait/advance-trait.md) | [06 方法、泛型、Trait 与特征对象](01-basics/06-methods-and-traits.md) | 正文 |
| 038 | [· 集合类型](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/collections/intro.md) | [07 Vec、HashMap、HashSet 与集合遍历](01-basics/07-collections.md) | 目录/导读 |
| 039 | [· · 动态数组 Vector](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/collections/vector.md) | [07 Vec、HashMap、HashSet 与集合遍历](01-basics/07-collections.md) | 正文 |
| 040 | [· · KV 存储 HashMap](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/collections/hashmap.md) | [07 Vec、HashMap、HashSet 与集合遍历](01-basics/07-collections.md) | 正文 |
| 041 | [· 认识生命周期](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/lifetime.md) | [08 生命周期：说明引用来自哪里](01-basics/08-lifetimes.md) | 正文 |
| 042 | [· 返回值和错误处理](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/result-error/intro.md) | [09 Option、Result、问号与错误边界](01-basics/09-error-handling.md) | 目录/导读 |
| 043 | [· · panic! 深入剖析](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/result-error/panic.md) | [09 Option、Result、问号与错误边界](01-basics/09-error-handling.md) | 正文 |
| 044 | [· · 返回值 Result 和?](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/result-error/result.md) | [09 Option、Result、问号与错误边界](01-basics/09-error-handling.md) | 正文 |
| 045 | [· 包和模块](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/crate-module/intro.md) | [10 包、模块、可见性、文档与格式化输出](01-basics/10-modules-and-docs.md) | 目录/导读 |
| 046 | [· · 包 Crate](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/crate-module/crate.md) | [10 包、模块、可见性、文档与格式化输出](01-basics/10-modules-and-docs.md) | 正文 |
| 047 | [· · 模块 Module](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/crate-module/module.md) | [10 包、模块、可见性、文档与格式化输出](01-basics/10-modules-and-docs.md) | 正文 |
| 048 | [· · 使用 use 引入模块及受限可见性](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/crate-module/use.md) | [10 包、模块、可见性、文档与格式化输出](01-basics/10-modules-and-docs.md) | 正文 |
| 049 | [· 注释和文档](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/comment.md) | [10 包、模块、可见性、文档与格式化输出](01-basics/10-modules-and-docs.md) | 正文 |
| 050 | [· 格式化输出](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic/formatted-output.md) | [10 包、模块、可见性、文档与格式化输出](01-basics/10-modules-and-docs.md) | 正文 |
| 051 | [入门实战：文件搜索工具](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic-practice/intro.md) | [11 入门实战：做一个小型文本搜索器](01-basics/11-cli-practice.md) | 目录/导读 |
| 052 | [· 基本功能](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic-practice/base-features.md) | [11 入门实战：做一个小型文本搜索器](01-basics/11-cli-practice.md) | 正文 |
| 053 | [· 增加模块化和错误处理](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic-practice/refactoring.md) | [11 入门实战：做一个小型文本搜索器](01-basics/11-cli-practice.md) | 正文 |
| 054 | [· 测试驱动开发](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic-practice/tests.md) | [11 入门实战：做一个小型文本搜索器](01-basics/11-cli-practice.md) | 正文 |
| 055 | [· 使用环境变量](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic-practice/envs.md) | [11 入门实战：做一个小型文本搜索器](01-basics/11-cli-practice.md) | 正文 |
| 056 | [· 重定向错误信息的输出](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic-practice/stderr.md) | [11 入门实战：做一个小型文本搜索器](01-basics/11-cli-practice.md) | 正文 |
| 057 | [· 使用迭代器来改进程序(可选)](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/basic-practice/iterators.md) | [11 入门实战：做一个小型文本搜索器](01-basics/11-cli-practice.md) | 正文 |

## Rust 语言进阶学习

| 序号 | 教程条目（固定源码） | 本地笔记 | 原文状态 |
| --- | --- | --- | --- |
| 058 | [Rust 高级进阶](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/intro.md) | [12 深入生命周期与 `'static`](02-advanced/01-lifetimes-and-static.md) | 目录/导读 |
| 059 | [· 生命周期](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/lifetime/intro.md) | [12 深入生命周期与 `'static`](02-advanced/01-lifetimes-and-static.md) | 目录/导读 |
| 060 | [· · 深入生命周期](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/lifetime/advance.md) | [12 深入生命周期与 `'static`](02-advanced/01-lifetimes-and-static.md) | 正文 |
| 061 | [· · &'static 和 T: 'static](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/lifetime/static.md) | [12 深入生命周期与 `'static`](02-advanced/01-lifetimes-and-static.md) | 正文 |
| 062 | [· 函数式编程: 闭包、迭代器](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/functional-programing/intro.md) | [13 闭包与迭代器：读懂函数式管道](02-advanced/02-closures-and-iterators.md) | 目录/导读 |
| 063 | [· · 闭包 Closure](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/functional-programing/closure.md) | [13 闭包与迭代器：读懂函数式管道](02-advanced/02-closures-and-iterators.md) | 正文 |
| 064 | [· · 迭代器 Iterator](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/functional-programing/iterator.md) | [13 闭包与迭代器：读懂函数式管道](02-advanced/02-closures-and-iterators.md) | 正文 |
| 065 | [· 深入类型](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/into-types/intro.md) | [14 类型转换、newtype、别名与 DST](02-advanced/03-type-conversions.md) | 目录/导读 |
| 066 | [· · 类型转换](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/into-types/converse.md) | [14 类型转换、newtype、别名与 DST](02-advanced/03-type-conversions.md) | 正文 |
| 067 | [· · newtype 和 类型别名](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/into-types/custom-type.md) | [14 类型转换、newtype、别名与 DST](02-advanced/03-type-conversions.md) | 正文 |
| 068 | [· · Sized 和不定长类型 DST](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/into-types/sized.md) | [14 类型转换、newtype、别名与 DST](02-advanced/03-type-conversions.md) | 正文 |
| 069 | [· · 枚举和整数](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/into-types/enum-int.md) | [14 类型转换、newtype、别名与 DST](02-advanced/03-type-conversions.md) | 正文 |
| 070 | [· 智能指针](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/smart-pointer/intro.md) | [15 智能指针：Box、Rc、Arc、Cell 与 RefCell](02-advanced/04-smart-pointers.md) | 目录/导读 |
| 071 | [· · Box&lt;T&gt; 堆对象分配](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/smart-pointer/box.md) | [15 智能指针：Box、Rc、Arc、Cell 与 RefCell](02-advanced/04-smart-pointers.md) | 正文 |
| 072 | [· · Deref 解引用](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/smart-pointer/deref.md) | [15 智能指针：Box、Rc、Arc、Cell 与 RefCell](02-advanced/04-smart-pointers.md) | 正文 |
| 073 | [· · Drop 释放资源](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/smart-pointer/drop.md) | [15 智能指针：Box、Rc、Arc、Cell 与 RefCell](02-advanced/04-smart-pointers.md) | 正文 |
| 074 | [· · Rc 与 Arc 实现 1vN 所有权机制](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/smart-pointer/rc-arc.md) | [15 智能指针：Box、Rc、Arc、Cell 与 RefCell](02-advanced/04-smart-pointers.md) | 正文 |
| 075 | [· · Cell 与 RefCell 内部可变性](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/smart-pointer/cell-refcell.md) | [15 智能指针：Box、Rc、Arc、Cell 与 RefCell](02-advanced/04-smart-pointers.md) | 正文 |
| 076 | [· 循环引用与自引用](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/circle-self-ref/intro.md) | [16 循环引用、自引用与 Weak](02-advanced/05-cycles-and-self-reference.md) | 目录/导读 |
| 077 | [· · Weak 与循环引用](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/circle-self-ref/circle-reference.md) | [16 循环引用、自引用与 Weak](02-advanced/05-cycles-and-self-reference.md) | 正文 |
| 078 | [· · 结构体中的自引用](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/circle-self-ref/self-referential.md) | [16 循环引用、自引用与 Weak](02-advanced/05-cycles-and-self-reference.md) | 正文 |
| 079 | [· 多线程并发编程](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/concurrency-with-threads/intro.md) | [17 线程、消息、锁、原子操作与 Send/Sync](02-advanced/06-threads-and-synchronization.md) | 目录/导读 |
| 080 | [· · 并发和并行](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/concurrency-with-threads/concurrency-parallelism.md) | [17 线程、消息、锁、原子操作与 Send/Sync](02-advanced/06-threads-and-synchronization.md) | 正文 |
| 081 | [· · 使用多线程](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/concurrency-with-threads/thread.md) | [17 线程、消息、锁、原子操作与 Send/Sync](02-advanced/06-threads-and-synchronization.md) | 正文 |
| 082 | [· · 线程同步：消息传递](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/concurrency-with-threads/message-passing.md) | [17 线程、消息、锁、原子操作与 Send/Sync](02-advanced/06-threads-and-synchronization.md) | 正文 |
| 083 | [· · 线程同步：锁、Condvar 和信号量](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/concurrency-with-threads/sync1.md) | [17 线程、消息、锁、原子操作与 Send/Sync](02-advanced/06-threads-and-synchronization.md) | 正文 |
| 084 | [· · 线程同步：Atomic 原子操作与内存顺序](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/concurrency-with-threads/sync2.md) | [17 线程、消息、锁、原子操作与 Send/Sync](02-advanced/06-threads-and-synchronization.md) | 正文 |
| 085 | [· · 基于 Send 和 Sync 的线程安全](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/concurrency-with-threads/send-sync.md) | [17 线程、消息、锁、原子操作与 Send/Sync](02-advanced/06-threads-and-synchronization.md) | 正文 |
| 086 | [· 全局变量](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/global-variable.md) | [18 全局变量与惰性初始化](02-advanced/07-globals.md) | 正文 |
| 087 | [· 错误处理](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/errors.md) | [09 Option、Result、问号与错误边界](01-basics/09-error-handling.md) | 正文 |
| 088 | [· Unsafe Rust](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/unsafe/intro.md) | [19 Unsafe、FFI 与内联汇编：先认识边界](02-advanced/08-unsafe-and-ffi.md) | 目录/导读 |
| 089 | [· · 五种兵器](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/unsafe/superpowers.md) | [19 Unsafe、FFI 与内联汇编：先认识边界](02-advanced/08-unsafe-and-ffi.md) | 正文 |
| 090 | [· · 内联汇编](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/unsafe/inline-asm.md) | [19 Unsafe、FFI 与内联汇编：先认识边界](02-advanced/08-unsafe-and-ffi.md) | 正文 |
| 091 | [· Macro 宏编程](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/macro.md) | [20 宏：先读懂，再考虑编写](02-advanced/09-macros.md) | 正文 |
| 092 | [· async/await 异步编程](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/async/intro.md) | [21 async、Future、运行时与 Pin](02-advanced/10-async-future-and-pin.md) | 目录/导读 |
| 093 | [· · async 编程入门](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/async/getting-started.md) | [21 async、Future、运行时与 Pin](02-advanced/10-async-future-and-pin.md) | 正文 |
| 094 | [· · 底层探秘: Future 执行与任务调度](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/async/future-excuting.md) | [21 async、Future、运行时与 Pin](02-advanced/10-async-future-and-pin.md) | 正文 |
| 095 | [· · 定海神针 Pin 和 Unpin](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/async/pin-unpin.md) | [21 async、Future、运行时与 Pin](02-advanced/10-async-future-and-pin.md) | 正文 |
| 096 | [· · async/await 和 Stream 流处理](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/async/async-await.md) | [22 Stream、并发组合、超时与取消](02-advanced/11-streams-and-cancellation.md) | 正文 |
| 097 | [· · 同时运行多个 Future](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/async/multi-futures-simultaneous.md) | [22 Stream、并发组合、超时与取消](02-advanced/11-streams-and-cancellation.md) | 正文 |
| 098 | [· · 一些疑难问题的解决办法](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/async/pain-points-and-workarounds.md) | [21 async、Future、运行时与 Pin](02-advanced/10-async-future-and-pin.md) | 正文 |
| 099 | [· · 实践应用：Async Web 服务器](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/async/web-server.md) | [23 Web 服务器实战：从串行到并发，再到关闭](02-advanced/12-web-server.md) | 正文 |
| 100 | [进阶实战1: 实现一个 web 服务器](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice1/intro.md) | [23 Web 服务器实战：从串行到并发，再到关闭](02-advanced/12-web-server.md) | 目录/导读 |
| 101 | [· 单线程版本](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice1/web-server.md) | [23 Web 服务器实战：从串行到并发，再到关闭](02-advanced/12-web-server.md) | 正文 |
| 102 | [· 多线程版本](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice1/multi-threads.md) | [23 Web 服务器实战：从串行到并发，再到关闭](02-advanced/12-web-server.md) | 正文 |
| 103 | [· 优雅关闭和资源清理](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice1/graceful-shutdown.md) | [23 Web 服务器实战：从串行到并发，再到关闭](02-advanced/12-web-server.md) | 正文 |
| 104 | [进阶实战2: 实现一个简单 Redis](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/intro.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 目录/导读 |
| 105 | [· tokio 概览](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/overview.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 正文 |
| 106 | [· 使用初印象](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/getting-startted.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 正文 |
| 107 | [· 创建异步任务](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/spawning.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 正文 |
| 108 | [· 共享状态](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/shared-state.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 正文 |
| 109 | [· 消息传递](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/channels.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 正文 |
| 110 | [· I/O](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/io.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 正文 |
| 111 | [· 解析数据帧](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/frame.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 正文 |
| 112 | [· 深入 async](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/async.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 正文 |
| 113 | [· select](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/select.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 正文 |
| 114 | [· 类似迭代器的 Stream](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/stream.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 正文 |
| 115 | [· 优雅的关闭](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/graceful-shutdown.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 正文 |
| 116 | [· 异步跟同步共存](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance-practice/bridging-with-sync.md) | [24 Tokio 与 mini-Redis 逐节导读](02-advanced/13-tokio-redis.md) | 正文 |
| 117 | [Rust 难点攻关](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/difficulties/intro.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 目录/导读 |
| 118 | [· 切片和切片引用](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/difficulties/slice.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 119 | [· Eq 和 PartialEq](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/difficulties/eq.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 120 | [· String、&str 和 str TODO](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/difficulties/string.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | TODO/doing，已有内容 |
| 121 | [· 作用域、生命周期和 NLL TODO](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/difficulties/lifetime.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 占位/外链；本地补充 |
| 122 | [· move、Copy 和 Clone TODO](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/difficulties/move-copy.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 占位/外链；本地补充 |
| 123 | [· 裸指针、引用和智能指针 TODO](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/advance/difficulties/pointer.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 占位/外链；本地补充 |

## 常用工具链

| 序号 | 教程条目（固定源码） | 本地笔记 | 原文状态 |
| --- | --- | --- | --- |
| 124 | [自动化测试](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/test/intro.md) | [25 自动化测试：验证可见行为](03-engineering/01-tests.md) | 目录/导读 |
| 125 | [· 编写测试及控制执行](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/test/write-tests.md) | [25 自动化测试：验证可见行为](03-engineering/01-tests.md) | 正文 |
| 126 | [· 单元测试和集成测试](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/test/unit-integration-test.md) | [25 自动化测试：验证可见行为](03-engineering/01-tests.md) | 正文 |
| 127 | [· 断言 assertion](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/test/assertion.md) | [25 自动化测试：验证可见行为](03-engineering/01-tests.md) | 正文 |
| 128 | [· 用 GitHub Actions 进行持续集成](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/test/ci.md) | [25 自动化测试：验证可见行为](03-engineering/01-tests.md) | 正文 |
| 129 | [· 基准测试 benchmark](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/test/benchmark.md) | [25 自动化测试：验证可见行为](03-engineering/01-tests.md) | 正文 |
| 130 | [Cargo 使用指南](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/intro.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 目录/导读 |
| 131 | [· 上手使用](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/getting-started.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 132 | [· 基础指南](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/guide/intro.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 目录/导读 |
| 133 | [· · 为何会有 Cargo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/guide/why-exist.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 134 | [· · 下载并构建 Package](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/guide/download-package.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 135 | [· · 添加依赖](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/guide/dependencies.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 136 | [· · Package 目录结构](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/guide/package-layout.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 137 | [· · Cargo.toml vs Cargo.lock](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/guide/cargo-toml-lock.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 138 | [· · 测试和 CI](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/guide/tests-ci.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 139 | [· · Cargo 缓存](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/guide/cargo-cache.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 140 | [· · Build 缓存](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/guide/build-cache.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 141 | [· 进阶指南](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/intro.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 目录/导读 |
| 142 | [· · 指定依赖项](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/specify-deps.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 143 | [· · 依赖覆盖](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/deps-overriding.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 144 | [· · Cargo.toml 清单详解](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/manifest.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 145 | [· · Cargo Target](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/cargo-target.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 146 | [· · 工作空间 Workspace](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/workspaces.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 147 | [· · 条件编译 Features](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/features/intro.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 目录/导读 |
| 148 | [· · · Features 示例](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/features/examples.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 149 | [· · 发布配置 Profile](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/profiles.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 150 | [· · 通过 config.toml 对 Cargo 进行配置](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/configuration.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 151 | [· · 发布到 crates.io](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/publishing-on-crates.io.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |
| 152 | [· · 构建脚本 build.rs](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/build-script/intro.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 目录/导读 |
| 153 | [· · · 构建脚本示例](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/cargo/reference/build-script/examples.md) | [26 Cargo 工程指南](03-engineering/02-cargo.md) | 正文 |

## 开发实践

| 序号 | 教程条目（固定源码） | 本地笔记 | 原文状态 |
| --- | --- | --- | --- |
| 154 | [企业落地实践](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/usecases/intro.md) | [28 开发技巧、生态选择与企业案例怎么读](03-engineering/04-practices-and-ecosystem.md) | 目录/导读 |
| 155 | [· AWS 为何这么喜欢 Rust?](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/usecases/aws-rust.md) | [28 开发技巧、生态选择与企业案例怎么读](03-engineering/04-practices-and-ecosystem.md) | 正文 |
| 156 | [日志和监控](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/logs/intro.md) | [27 日志、可观测性与项目的数据库 Trace](03-engineering/03-logging-and-tracing.md) | 目录/导读 |
| 157 | [· 日志详解](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/logs/about-log.md) | [27 日志、可观测性与项目的数据库 Trace](03-engineering/03-logging-and-tracing.md) | 正文 |
| 158 | [· 日志门面 log](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/logs/log.md) | [27 日志、可观测性与项目的数据库 Trace](03-engineering/03-logging-and-tracing.md) | 正文 |
| 159 | [· 使用 tracing 记录日志](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/logs/tracing.md) | [27 日志、可观测性与项目的数据库 Trace](03-engineering/03-logging-and-tracing.md) | 正文 |
| 160 | [· 自定义 tracing 的输出格式](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/logs/tracing-logger.md) | [27 日志、可观测性与项目的数据库 Trace](03-engineering/03-logging-and-tracing.md) | 正文 |
| 161 | [· 监控](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/logs/observe/intro.md) | [27 日志、可观测性与项目的数据库 Trace](03-engineering/03-logging-and-tracing.md) | 目录/导读 |
| 162 | [· · 可观测性](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/logs/observe/about-observe.md) | [27 日志、可观测性与项目的数据库 Trace](03-engineering/03-logging-and-tracing.md) | 正文 |
| 163 | [· · 分布式追踪](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/logs/observe/trace.md) | [27 日志、可观测性与项目的数据库 Trace](03-engineering/03-logging-and-tracing.md) | 占位/外链；本地补充 |
| 164 | [Rust 最佳实践](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/practice/intro.md) | [28 开发技巧、生态选择与企业案例怎么读](03-engineering/04-practices-and-ecosystem.md) | 目录/导读 |
| 165 | [· 日常开发三方库精选](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/practice/third-party-libs.md) | [28 开发技巧、生态选择与企业案例怎么读](03-engineering/04-practices-and-ecosystem.md) | 正文 |
| 166 | [· 命名规范](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/practice/naming.md) | [28 开发技巧、生态选择与企业案例怎么读](03-engineering/04-practices-and-ecosystem.md) | 正文 |
| 167 | [· 面试经验](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/practice/interview.md) | [28 开发技巧、生态选择与企业案例怎么读](03-engineering/04-practices-and-ecosystem.md) | 正文 |
| 168 | [· 代码开发实践 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/practice/best-pratice.md) | [28 开发技巧、生态选择与企业案例怎么读](03-engineering/04-practices-and-ecosystem.md) | TODO/doing，已有内容 |
| 169 | [手把手带你实现链表](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/intro.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 目录/导读 |
| 170 | [· 我们到底需不需要链表](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/do-we-need-it.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 171 | [· 不太优秀的单向链表：栈](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/bad-stack/intro.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 目录/导读 |
| 172 | [· · 数据布局](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/bad-stack/layout.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 173 | [· · 基本操作](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/bad-stack/basic-operations.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 174 | [· · 最后实现](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/bad-stack/final-code.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 175 | [· 还可以的单向链表](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/ok-stack/intro.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 目录/导读 |
| 176 | [· · 优化类型定义](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/ok-stack/type-optimizing.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 177 | [· · 定义 Peek 函数](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/ok-stack/peek.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 178 | [· · IntoIter 和 Iter](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/ok-stack/iter.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 179 | [· · IterMut 以及完整代码](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/ok-stack/itermut.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 180 | [· 持久化单向链表](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/persistent-stack/intro.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 目录/导读 |
| 181 | [· · 数据布局和基本操作](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/persistent-stack/layout.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 182 | [· · Drop、Arc 及完整代码](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/persistent-stack/drop-arc.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 183 | [· 不咋样的双端队列](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/deque/intro.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 目录/导读 |
| 184 | [· · 数据布局和基本操作](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/deque/layout.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 185 | [· · Peek](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/deque/peek.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 186 | [· · 基本操作的对称镜像](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/deque/symmetric.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 187 | [· · 迭代器](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/deque/iterator.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 188 | [· · 最终代码](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/deque/final-code.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 189 | [· 不错的 unsafe 队列](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/unsafe-queue/intro.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 目录/导读 |
| 190 | [· · 数据布局](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/unsafe-queue/layout.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 191 | [· · 基本操作](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/unsafe-queue/basics.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 192 | [· · Miri](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/unsafe-queue/miri.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 193 | [· · 栈借用](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/unsafe-queue/stacked-borrow.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 194 | [· · 测试栈借用](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/unsafe-queue/testing-stacked-borrow.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 195 | [· · 数据布局 2](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/unsafe-queue/layout2.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 196 | [· · 额外的操作](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/unsafe-queue/extra-junk.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 197 | [· · 最终代码](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/unsafe-queue/final-code.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 198 | [· 生产级的双向 unsafe 队列](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/production-unsafe-deque/intro.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 目录/导读 |
| 199 | [· · 数据布局](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/production-unsafe-deque/layout.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 200 | [· · 型变与子类型](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/production-unsafe-deque/variance-and-phantomData.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 201 | [· · 基础结构](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/production-unsafe-deque/basics.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 202 | [· · 恐慌与安全](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/production-unsafe-deque/drop-and-panic-safety.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 203 | [· · 无聊的组合](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/production-unsafe-deque/boring-combinatorics.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 204 | [· · 其它特征](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/production-unsafe-deque/filling-in-random-bits.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 205 | [· · 测试](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/production-unsafe-deque/testing.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 206 | [· · Send,Sync和编译测试](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/production-unsafe-deque/send-sync-and-compile-tests.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 207 | [· · 实现游标](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/production-unsafe-deque/implementing-cursors.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 208 | [· · 测试游标](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/production-unsafe-deque/testing-cursors.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 209 | [· · 最终代码](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/production-unsafe-deque/final-code.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 210 | [· 使用高级技巧实现链表](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/advanced-lists/intro.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 目录/导读 |
| 211 | [· · 双单向链表](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/advanced-lists/double-singly.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |
| 212 | [· · 栈上的链表](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/too-many-lists/advanced-lists/stack-allocated.md) | [29 链表系列：按阶段理解所有权设计](03-engineering/05-linked-lists.md) | 正文 |

## 攻克编译错误

| 序号 | 教程条目（固定源码） | 本地笔记 | 原文状态 |
| --- | --- | --- | --- |
| 213 | [征服编译错误](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/intro.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 占位/外链；本地补充 |
| 214 | [· 对抗编译检查](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/fight-with-compiler/intro.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 占位/外链；本地补充 |
| 215 | [· · 生命周期](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/fight-with-compiler/lifetime/intro.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 目录/导读 |
| 216 | [· · · 生命周期过大-01](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/fight-with-compiler/lifetime/too-long1.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 217 | [· · · 生命周期过大-02](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/fight-with-compiler/lifetime/too-long2.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 218 | [· · · 循环中的生命周期](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/fight-with-compiler/lifetime/loop.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 219 | [· · · 闭包碰到特征对象-01](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/fight-with-compiler/lifetime/closure-with-static.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 220 | [· · 重复借用](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/fight-with-compiler/borrowing/intro.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 目录/导读 |
| 221 | [· · · 同时在函数内外使用引用](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/fight-with-compiler/borrowing/ref-exist-in-out-fn.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 222 | [· · · 智能指针引起的重复借用错误](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/fight-with-compiler/borrowing/borrow-distinct-fields-of-struct.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 223 | [· · 类型未限制(todo)](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/fight-with-compiler/unconstrained.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 占位/外链；本地补充 |
| 224 | [· · 幽灵数据(todo)](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/fight-with-compiler/phantom-data.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 占位/外链；本地补充 |
| 225 | [· Rust 常见陷阱](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/pitfalls/index.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 目录/导读 |
| 226 | [· · for 循环中使用外部数组](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/pitfalls/use-vec-in-for.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 227 | [· · 线程类型导致的栈溢出](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/pitfalls/stack-overflow.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 228 | [· · 算术溢出导致的 panic](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/pitfalls/arithmetic-overflow.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 229 | [· · 闭包中奇怪的生命周期](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/pitfalls/closure-with-lifetime.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 230 | [· · 可变变量不可变？](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/pitfalls/the-disabled-mutability.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 231 | [· · 可变借用失败引发的深入思考](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/pitfalls/multiple-mutable-references.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 232 | [· · 不太勤快的迭代器](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/pitfalls/lazy-iterators.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 233 | [· · 奇怪的序列 x..y](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/pitfalls/weird-ranges.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 占位/外链；本地补充 |
| 234 | [· · 无处不在的迭代器](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/pitfalls/iterator-everywhere.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 235 | [· · 线程间传递消息导致主线程无法结束](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/pitfalls/main-with-channel-blocked.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |
| 236 | [· · 警惕 UTF-8 引发的性能隐患](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/compiler/pitfalls/utf8-performance.md) | [30 编译错误、难点与常见陷阱](03-engineering/06-compiler-and-pitfalls.md) | 正文 |

## 性能优化

| 序号 | 教程条目（固定源码） | 本地笔记 | 原文状态 |
| --- | --- | --- | --- |
| 237 | [Rust 性能优化 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/intro.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 238 | [· 深入内存 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/memory/intro.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 239 | [· · 指针和引用 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/memory/pointer-ref.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 240 | [· · 未初始化内存 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/memory/uninit.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 241 | [· · 内存分配 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/memory/allocation.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 242 | [· · 内存布局 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/memory/layout.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 243 | [· · 虚拟内存 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/memory/virtual.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 244 | [· 性能调优 doing](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/performance/intro.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | TODO/doing，已有内容 |
| 245 | [· · 字符串操作性能](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/performance/string.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 246 | [· · 深入理解 move](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/performance/deep-into-move.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 正文 |
| 247 | [· · 糟糕的提前优化 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/performance/early-optimise.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | TODO/doing，已有内容 |
| 248 | [· · Clone 和 Copy todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/performance/clone-copy.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 249 | [· · 减少 Runtime check(todo)](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/performance/runtime-check.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | TODO/doing，已有内容 |
| 250 | [· · CPU 缓存性能优化 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/performance/cpu-cache.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | TODO/doing，已有内容 |
| 251 | [· · 计算性能优化 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/performance/calculate.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | TODO/doing，已有内容 |
| 252 | [· · 堆和栈 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/performance/heap-stack.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 253 | [· · 内存 allocator todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/performance/allocator.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 254 | [· · 常用性能测试工具 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/performance/tools.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 255 | [· · Enum 内存优化 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/performance/enum.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 256 | [· 编译优化 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/compiler/intro.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 257 | [· · LLVM todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/compiler/llvm.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |
| 258 | [· · 常见属性标记 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/compiler/attributes.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | TODO/doing，已有内容 |
| 259 | [· · 提升编译速度 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/compiler/speed-up.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | TODO/doing，已有内容 |
| 260 | [· · 编译器优化 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/compiler/optimization/intro.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | TODO/doing，已有内容 |
| 261 | [· · · Option 枚举 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/profiling/compiler/optimization/option.md) | [31 性能：先测量，再解释成本](03-engineering/07-performance.md) | 占位/外链；本地补充 |

## 附录

| 序号 | 教程条目（固定源码） | 本地笔记 | 原文状态 |
| --- | --- | --- | --- |
| 262 | [· 关键字](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/keywords.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 263 | [· 运算符与符号](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/operators.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 264 | [· 表达式](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/expressions.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 265 | [· 派生特征 trait](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/derive.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 266 | [· prelude 模块 todo](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/prelude.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 占位/外链；本地补充 |
| 267 | [· Rust 版本说明](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-version.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 268 | [· Rust 历次版本更新解读](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/intro.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 目录/导读 |
| 269 | [· · 1.58](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.58.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 270 | [· · 1.59](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.59.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 271 | [· · 1.60](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.60.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 272 | [· · 1.61](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.61.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 273 | [· · 1.62](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.62.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 274 | [· · 1.63](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.63.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 275 | [· · 1.64](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.64.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 276 | [· · 1.65](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.65.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 277 | [· · 1.66](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.66.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 278 | [· · 1.67](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.67.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 279 | [· · 1.68](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.68.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 280 | [· · 1.69](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.69.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 281 | [· · 1.70](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.70.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 282 | [· · 1.71](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.71.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 283 | [· · 1.72](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.72.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 284 | [· · 1.73](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.73.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 285 | [· · 1.74](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.74.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 286 | [· · 1.75](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.75.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 287 | [· · 1.76](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.76.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 288 | [· · 1.77](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.77.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 289 | [· · 1.78](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.78.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 290 | [· · 1.79](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.79.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 291 | [· · 1.80](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.80.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 292 | [· · 1.81](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.81.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 293 | [· · 1.82](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.82.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 294 | [· · 1.83](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.83.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 295 | [· · 1.84](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.84.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 296 | [· · 1.85](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.85.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 297 | [· · 1.86](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.86.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 298 | [· · 1.87](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.87.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 299 | [· · 1.88](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.88.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |
| 300 | [· · 1.89](https://github.com/sunface/rust-course/blob/ebe2d82437f621085b0cb6895edbd1283ed63cb1/src/appendix/rust-versions/1.89.md) | [35 语法速查、派生特征与版本差异](05-reference/01-syntax-and-versions.md) | 正文 |

## 项目补充入口

原教程之外，增加 [源码地图](04-project/01-code-map.md)、[完整请求阅读](04-project/02-reading-a-request.md)、[分阶段练习](04-project/03-exercises.md) 和 [资料及验证说明](05-reference/02-resources-and-verification.md)。这些内容来自本项目和官方资料，不伪装为原书章节。
