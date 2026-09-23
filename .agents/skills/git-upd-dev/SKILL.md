---
name: git-upd-dev
description: Use when the user explicitly invokes `upd dev` to fast-forward local dev and main from origin, merge main into dev, push dev, and return to the original branch.
---

# Git upd dev

仅在用户明确输入 `upd dev` 时使用。从项目根目录运行：

```text
python -X utf8 .agents/skills/git-upd-dev/scripts/update_dev.py
```

脚本会记录当前分支并确认工作区干净，然后先将本地 `dev` 用 `git pull --ff-only origin dev` 快进到远端最新状态。若本地存在 `main`，再切到 `main` 并用 `git pull --ff-only origin main` 更新它，然后切回 `dev`；若没有本地 `main`，则 fetch `origin/main`。随后将最新的 `origin/main` 合并到 `dev`，避免把本地 `main` 独有的未推送提交带入 `dev`。合并后执行 `git push origin dev`，最后切回原分支。

`--ff-only` 遇到本地与远端分叉时会停止，不会擅自创建额外合并。脚本不 stash、reset、force-push 或替用户处理非冲突错误。

如果合并冲突：

1. 保持当前 `dev` 分支及进行中的 merge，不要 abort 或切换分支。
2. 使用项目内 `$resolving-merge-conflicts`（`.agents/skills/resolving-merge-conflicts/SKILL.md`）检查、解决、验证并提交冲突。
3. 在 `dev` 上重新运行上面的同一命令；脚本会沿用冲突前记录的原分支，push 后切回它。

冲突以外的失败先停止并报告实际 Git 状态。
