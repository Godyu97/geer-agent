# Design

## Context

参见 `proposal.md` 的 Why。当前启动路径分别被 `SessionRuntime`、`Tools` 和 `prompt::load` 捕获，三者之后没有统一更新入口；`SessionRecord` 已保存 workspace，DAO 目前只支持按单个目录列出。现有 `Prompt` 负责消息所有权，`SessionManager` 负责活动与暂存会话，Provider、工具注册表和数据库连接在会话间共享。

配置目录已经独立于启动 cwd 选定，默认 SQLite 也跟随配置目录。本变更必须保持这条规则，不能通过修改进程 cwd 实现 workspace 切换。

## Goals / Non-Goals

**Goals:**

- 建立单一、可校验的 workspace 值，并让每条会话拥有自己的值。
- 在完整用户轮次之间原子切换会话、提示词和工具执行目录。
- 沿用现有 Prompt、Provider、工具授权、DAO 和保存失败补写路径做增量扩展。
- 让三种界面共享命令语义，并只在各自展示层增加必要交互。

**Non-Goals:**

- workspace 不是安全沙箱，绝对路径和 `~/` 仍按现有工具规则可用。
- 不让 workspace 参与 `.env`、数据库或 Bash 可执行文件的配置选择。
- 不把三种界面合并为新的 UI 框架，也不增加后台切换或并发会话执行。

## Decisions

### 1. `Workspace` 是会话拥有的值对象

在 `src/session` 增加 `Workspace`，内部持有规范化的绝对 `PathBuf`，并把现有 `SessionState`、`SessionManager`、`SessionRuntime` 从 Agent 私有子模块移入 session 层。移动只改变归属，不改变 Prompt 或 DAO 的职责。

解析以当前 workspace 为相对基准；只处理一层成对的单/双引号和 `~`。Linux/macOS 使用 `HOME`，Windows 回退 `USERPROFILE`。随后调用标准库规范化并验证目录类型。所有失败返回中文 `Result`，调用方在替换活动状态前处理，因此不需要回滚半成品。

不选择直接保存字符串：不同写法和符号链接会造成同一目录被误判为切换。也不调用 `set_current_dir`：它是进程全局状态，会让配置、异步任务和未来并发会话相互影响。

### 2. 静态环境只探测一次，workspace 在创建 Prompt 时注入

`prompt` 把当前 `load` 拆成异步加载的静态环境对象和同步的 workspace 渲染。系统版本、Bash 版本在启动时探测一次；每次新建或恢复会话时，用目标 `Workspace` 组装完整系统提示。`Prompt` 继续独占 Chat/Responses 历史、摘要、待处理消息和显示事件。

不在每次模型请求临时拼接目录：系统提示也参与上下文估算和会话恢复，临时覆盖容易让估算与实际请求不一致。workspace 变更本来就创建新会话，因此每条 Prompt 的系统内容在生命周期内保持稳定。

### 3. 持久化模板不保存 workspace

`SessionRuntime` 保留共享的 store、接口、模型、端点、密钥脱敏信息和每会话 revision/事件队列，但移除固定 workspace。保存方法显式接收所属 `Workspace`；创建新运行状态时只复制共享配置和空的保存游标。`SessionRecord.workspace` 继续写规范化字符串，无需迁移。

恢复数据库会话时先按 ID 读取记录，再解析记录中的 workspace、验证其目录和其余兼容字段，构造完整目标状态后才切换。进程内暂存状态已经持有 `Workspace`，无需数据库也可往返。

DAO 增加“所有 workspace 的近期会话”查询：SQL 与 MongoDB 均按更新时间倒序取最多 20 条；现有按 workspace 查询继续保留。这样 `/sessions` 的范围和成本不变，`--all` 也有明确上限。

不在切换 workspace 时把旧会话的记录改写成新目录，因为这会把历史上下文错误归到另一个项目。

### 4. 工具执行显式接收 workspace

`Tools` 移除 cwd 字段，`execute_batch` 和单次执行入口接收 `&Path`。Agent 在每个完整工具批次开始前借用活动会话的 workspace 并传入；批次执行期间命令语义不允许切换 workspace，因此异步并行只读调用共享该不可变路径是安全的。Bash `current_dir`、文件解析和查询根目录沿用现有实现。

工具定义把“启动目录”改为“当前 workspace”，授权提示继续说明实际路径。成功切换或恢复不同会话后调用现有 `reset` 清空授权；同目录无操作和打开当前 UUID 不清空。

不把 workspace 再复制进 Tools：复制会重新引入三处状态同步问题。

### 5. Agent 负责原子协调，interaction 只定义公共语义

公共 `Input` 增加 `Workspace(Option<String>)`，`Sessions` 携带 current/all 范围；`Session` 契约增加读取、设置 workspace 及按范围列出会话。`SessionStatus`、`SessionEntry` 增加 workspace 字符串，使 TUI 和 GUI 不直接读取 session 内部。

设置流程为：解析并验证目录 → 由 `SessionManager` 准备新 Prompt 和运行状态 → 尝试保存旧会话 → 替换活动状态并暂存旧状态 → Agent 清授权和本轮用量。旧会话保存失败只产生既有诊断，队列仍随旧状态保存。

打开流程先完整准备内存或数据库目标，再保存并替换当前状态。返回值区分无操作、普通切换和工具状态未确认，Agent 据此更新授权与反馈。

### 6. 三端只增加各自需要的展示状态

REPL 直接处理公共命令。TUI 增加 `Workspace` 输入模式和独立输入缓冲；F2 在聊天模式进入，Esc 恢复聊天缓冲，错误不退出该模式。状态面板显示路径；窄屏输入标题包含 `F2 workspace` 提示。

GUI 快照同时携带当前范围和全部范围的会话列表，前端本地切换筛选，不需要额外往返。workspace 表单提交 `/workspace <path>`，复用工作线程的忙碌锁和快照更新。系统目录选择使用 `tauri-plugin-dialog`，因为标准库和 Tauri core 不提供跨平台原生目录选择器；依赖只挂在现有 `gui` feature，并只给 `dialog:allow-open`。浏览结果只写入表单，确认仍走公共命令。

### 7. 错误、配置与流式边界

路径与恢复错误通过已有命令结果返回用户；存储错误继续走诊断缓冲并允许对话。workspace 不进入配置加载，也不改变数据库 URL。模型流式轮次和工具授权期间，REPL/TUI 的输入循环以及 GUI 的 busy 状态都不会调度 workspace 命令，因此不存在半轮切换；不新增锁或取消机制。

## Risks / Trade-offs

- [全部 workspace 查询在大型远程数据库中可能较慢] → 两种后端都按已有更新时间索引排序并限制 20 条。
- [规范化会解析符号链接，显示路径可能不同于用户输入] → 用规范化路径做稳定身份，界面始终显示最终路径。
- [恢复会话时目录已被删除] → 在切换前拒绝并保留当前状态，不自动回退到父目录。
- [GUI 新插件扩大可选构建依赖] → 只在 `gui` feature 初始化，默认构建不包含；权限只开放目录选择所需的 open。
- [移动会话代码会扩大 diff] → 保持现有类型和算法，先机械迁移并通过现有测试，再加入 workspace 行为。

## Migration Plan

1. 先引入 `Workspace` 并迁移会话模块，保持启动目录行为等价。
2. 将 Prompt 和 Tools 改为显式 workspace，再接入命令和会话恢复。
3. 增加 TUI 与 GUI 入口及可选 dialog 依赖。
4. 既有记录无需转换；其 workspace 在恢复时按新规则规范化和验证。回滚代码后数据库仍兼容，因为字段格式未变。

## Open Questions

- Windows 原生目录选择对话框未在本机实测。Linux 已通过 `tauri-plugin-dialog` 接入；Windows 行为依赖该插件的平台实现，验收时需确认 `dialog:allow-open` 选中的目录只写入表单、确认仍走 `/workspace <path>`。
