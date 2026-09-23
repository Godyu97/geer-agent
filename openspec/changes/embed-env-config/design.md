# Design

## Context

`src/config/mod.rs` 目前检查当前目录的 `.env`，再用 `dotenvy::dotenv()` 将文件中未在进程环境出现的值载入。配置校验已独立于读取过程。

## Decisions

- 在 Cargo 中增加默认关闭的 `embed-env` feature，使用 Rust 编译期文件包含机制把仓库根目录 `.env` 放入二进制。缺文件时编译器直接报错，不增加 build script 或新 crate。
- 先加载运行目录 `.env`，再用已有 `dotenvy` 的 reader 接口加载内嵌文本。该接口只补充环境中不存在的键，因此优先级为进程环境、运行目录文件、内嵌内容；普通构建的路径保持原样。
- 仍由 `Config::from_values` 校验必需项与默认 endpoint。文本由静态借用传给 reader，无额外所有权或生命周期结构。加载失败沿现有 `Result` 路径退出，和异步流式请求无关。
- 标准库支持文件嵌入，但不解析 dotenv 语法；继续复用已经存在的 `dotenvy`，无需新依赖。Rust 编译条件映射教程里的可选配置来源，而非新增模块或框架。

## Risks / Trade-offs

- 内嵌 `.env` 原文可从二进制提取，因此只适合用户自己控制分发范围的产物；说明文档明确这一点。
- 切换 `.env` 内容需要重新构建；运行目录的文件或进程环境可在不重编译时覆盖配置。

## Open Questions

无。
