# 审查训练：找出能编译但不可靠的设计

[返回总目录](../README.md) · [上一篇](04-capstone-and-rubric.md)

每题先指出失败输入或执行交错，再给出修改和验证证据。编译通过只排除一部分问题，资源、业务与协议仍要审查。

## 题一：配置错误变成默认值

```rust
fn bad_limit(value: Option<&str>) -> usize {
    value.and_then(|text| text.parse().ok()).unwrap_or(16)
}

fn main() {
    assert_eq!(bad_limit(Some("wrong")), 16);
}
```

未设置和非法值被合并，用户以为配置生效。改为 Result，仅 None 走默认，空值、非法数字与越界分别检查。[迁移练习](../06-go-to-rust/05-migration-workshop.md)。

## 题二：按字符数切字符串

故意的运行失败例子：

```rust,should_panic
fn main() {
    let text = "学习Rust";
    let _preview = &text[..1];
}
```

1 是字节位置，落在字符内部。改为 char_indices、get 或业务要求的字素簇处理，验证中文、组合字符与 emoji，并说明截断单位。[文本](../07-standard-library/02-collections-and-text.md)。

## 题三：clone 改变了状态归属

每请求克隆整个 session，再修改克隆对象，可能每次返回成功但真实状态没有变化。Arc clone 和深克隆也不同。先写拥有者，再选独占借用、单拥有者或共享锁；确实需要快照才克隆。验证连续请求和并发交错。[所有权 API](../06-go-to-rust/02-ownership-and-api.md)。

## 题四：timeout 自动取消后台任务

需要 Tokio 上下文的错误设计：

```rust,ignore
let handle = tokio::spawn(work());
let _ = tokio::time::timeout(duration, handle).await;
```

超时丢弃拥有的 JoinHandle，任务可能继续。保留句柄，明确 abort/合作取消并 await；外部提交还需处理结果未知。验证未开始、运行中、已提交阶段。[Tokio](../08-ecosystem/03-tokio-runtime.md)。

## 题五：spawn 后才获取 Semaphore

```text
for 每个输入:
    spawn 任务
    任务内部等待 semaphore
```

只限制许可，没有限制已创建任务。获得容量后再创建，或有限 worker/JoinSet 逐项补充；消息与结果另设上限。用有界的大批次记录在途与排队峰值。

## 题六：锁跨 await

可编译的异步锁仍可能令全服务等待一个慢上游。另一任务若要这把锁发取消或授权，可能相互等待。缩短锁范围，I/O 后检查 revision 再提交；或用单拥有者和独立唤醒通道。[AppRuntime](../../../src/ui/app/runtime.rs)。

## 题七：HashMap 顺序成为协议

小测试可能偶然看到稳定次序。明确排序或用 BTreeMap，定义并列 tie-break；数据库批量结果也要按请求 ID 归位。[集合](../07-standard-library/02-collections-and-text.md)。

## 题八：编译成功等于跨平台交付

链接不能证明目标机有动态库/WebView，或路径、子进程与窗口行为正确。分别记录 host 构建、target 产物与目标运行，区分手工验收和自动测试。[交付](../03-engineering/13-workspace-ci-and-release.md)。

## 题九：返回全部内部 Debug

数据库 URL、路径、模型请求和 token 可能泄漏；客户端也无法区分错误类别。内部保留脱敏上下文，对外给稳定错误码、提示与操作 ID。[服务工程](../03-engineering/11-production-services.md)。

## 题十：测试析构就能清理全部资源

panic、超时、SIGKILL、脱离进程组的后代与临时文件都可能越过析构/trap。内层进程组清理与 wait，外层 cgroup 和私有 tmpfs，各有职责。核对真实限制，入口失败不得绕过。[AGENTS.md](../../../AGENTS.md)。

## 审查记录

```text
触发：输入、失败路径或执行交错
结果：违反的行为、资源或协议约束
原因：所有权、状态、调度、I/O 或存储
修改：最小修复与理由
验证：有限可重复的成功、失败、边界证据
限制：未覆盖的平台与保证
```

每次审查至少检查一个状态或资源边界，不只处理格式和命名；未发现问题时也应能说明审查范围。
