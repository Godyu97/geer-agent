# S3 I/O、路径与进程：边界要明确且有上限

[返回总目录](../README.md) · [上一篇](02-collections-and-text.md) · [下一篇](04-time-sync-and-utilities.md)

## Read 和 Write 操作的是字节，不是完整消息

`read` 一次成功可只读一部分，`write` 也可只写一部分。需要固定数量用 read_exact，需要写完整缓冲用 write_all；文件、TCP 都不能假定一次调用完成一个业务包。对非空缓冲，普通 Read 的 `Ok(0)` 通常表示当前到达 EOF；空缓冲本身也可能返回零。[Read](https://doc.rust-lang.org/std/io/trait.Read.html)、[Write](https://doc.rust-lang.org/std/io/trait.Write.html)。

```rust
use std::io::{self, Read};

fn read_limited(reader: impl Read, max_bytes: usize) -> io::Result<Vec<u8>> {
    let limit = u64::try_from(max_bytes)
        .ok().and_then(|n| n.checked_add(1))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "limit too large"))?;
    let mut data = Vec::new();
    reader.take(limit).read_to_end(&mut data)?;
    if data.len() > max_bytes {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "input too large"));
    }
    Ok(data)
}

fn main() -> io::Result<()> {
    assert_eq!(read_limited(&b"hello"[..], 5)?, b"hello");
    assert!(read_limited(&b"hello!"[..], 5).is_err());
    Ok(())
}
```

多读一个字节用于辨别“恰好达到上限”和“超过上限”。这只限制读取量，不限制阻塞等待时间；如果 reader 是 socket，还要配置超时或使用有超时预算的异步路径。

## 缓冲与 framing 是不同层

BufReader 减少底层读取次数，BufWriter 减少底层写入次数；它们不负责业务消息边界。按行、长度前缀或分隔符解析才是 framing。`lines()` 方便，但单行长度仍可能无界，TCP 对端也可能永远不发换行。

```mermaid
flowchart LR
    A[操作系统字节流] --> B[有字节和时间限制的缓冲]
    B --> C[帧解析：处理不足与多帧]
    C --> D[UTF-8 或结构化解码]
    D --> E[业务验证与状态转换]
```

BufWriter 的 Drop 不能可靠报告写回失败，重要写入应显式 flush 并检查结果；flush 不等于数据已经耐久落盘，持久化要求还涉及 sync 和文件系统契约。[BufWriter](https://doc.rust-lang.org/std/io/struct.BufWriter.html)、[File::sync_all](https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_all)。

## Path/PathBuf 对应 String/&str 的拥有关系

| 文本 | 路径/平台字符串 | 含义 |
| --- | --- | --- |
| String | PathBuf / OsString | 拥有可保存的数据 |
| str / &str | Path / &Path、OsStr / &OsStr | 借用视图 |

路径不保证 UTF-8，日志可用 display，业务不应强制 `to_str().unwrap()`。`join` 在右侧是绝对路径时可能替换前面的部分，不能把字符串拼接理解为路径安全。[path](https://doc.rust-lang.org/std/path/index.html)、[PathBuf::push](https://doc.rust-lang.org/std/path/struct.PathBuf.html#method.push)。

规范化字符串、canonicalize 和最终打开文件解决不同问题：路径中存在符号链接，检查后路径也可能变化。需要限制工具到 workspace 时，应明确威胁模型、最终目标验证和并发变化处理。项目例子见 [tools/file.rs](../../../src/tools/file.rs)；没有一个简单 `starts_with` 可以证明所有平台上的文件访问安全。

## 子进程是一组资源

创建 Command 时明确四件事：可执行文件、独立参数、环境、标准输入输出。直接 `.arg(value)` 是传入一个参数；`bash -c` 或命令行字符串则进入 shell 解析，需要另一套授权与转义边界。[Command](https://doc.rust-lang.org/std/process/struct.Command.html)。

| 资源 | 必须决定的策略 |
| --- | --- |
| stdin | 无输入时 null，有输入时管道并显式 EOF |
| stdout/stderr | 同时读取、各有上限，避免管道满导致相互等待 |
| 时间 | 总执行期限和停止宽限期 |
| 后代 | 单进程、进程组与 cgroup 的清理范围 |
| 主进程 | 停止后 wait 回收；Drop 不等于完整清理 |
| 临时文件 | 属于本轮隔离并随服务回收 |

标准库 Child 的 Drop 不会自动 wait 回收；需要显式生命周期管理。[Child](https://doc.rust-lang.org/std/process/struct.Child.html)。本仓库的完整约束见 [AGENTS.md](../../../AGENTS.md)，测试进程统一使用 [tests/support](../../../tests/support/mod.rs) 与受限入口，不能裸执行危险复现。

## 文件写入的工程流程

```text
解析输入 → 验证目标 → 写本轮临时文件 → 检查 flush/sync 要求
→ 在允许的同文件系统语义下替换 → 报告最终结果
```

原子 rename、覆盖行为和耐久性依赖平台与文件系统，不能把“rename 成功”泛化为断电后一定保存。并发写同一个路径还需要冲突或版本策略。[fs::rename](https://doc.rust-lang.org/std/fs/fn.rename.html)。

验收：实现有界文本读取器，至少覆盖空输入、刚好上限、超一字节、非法 UTF-8、读失败。用纯内存 reader 注入故障，不必为此启动外部程序。
