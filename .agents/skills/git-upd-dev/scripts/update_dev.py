#!/usr/bin/env python3
"""Update dev and main from origin, merge main into dev, push, and return."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Sequence


REMOTE = "origin"
MAIN_BRANCH = "main"
DEV_BRANCH = "dev"
STATE_FILE_NAME = "git-upd-dev-state.json"
CONFLICT_EXIT = 3
ERROR_EXIT = 1
USAGE = "python -X utf8 .agents/skills/git-upd-dev/scripts/update_dev.py"


class GitError(RuntimeError):
    """A git command failed without starting a conflict operation."""

    def __init__(self, args: Sequence[str], returncode: int, stderr: str):
        self.args = list(args)
        self.returncode = returncode
        self.stderr = stderr.strip()
        command = "git " + " ".join(self.args)
        detail = self.stderr or f"exit code {returncode}"
        super().__init__(f"{command}: {detail}")


class ConflictPendingError(RuntimeError):
    """The saved workflow still has an unresolved merge or uncommitted fix."""


def run_git(*args: str, check: bool = True) -> str:
    result = subprocess.run(
        ["git", *args],
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if check and result.returncode != 0:
        raise GitError(args, result.returncode, result.stderr)
    return result.stdout.strip()


def git_path(*args: str) -> Path:
    return Path(run_git("rev-parse", "--git-path", *args))


def state_path() -> Path:
    path = git_path(STATE_FILE_NAME)
    if not path.is_absolute():
        path = Path.cwd() / path
    return path


def current_branch() -> str:
    branch = run_git("branch", "--show-current")
    if not branch:
        raise RuntimeError("当前处于 detached HEAD，无法记录原分支")
    return branch


def worktree_dirty() -> bool:
    return bool(run_git("status", "--porcelain=v1"))


def local_branch_exists(branch: str) -> bool:
    result = subprocess.run(
        ["git", "show-ref", "--verify", "--quiet", f"refs/heads/{branch}"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return result.returncode == 0


def operation_in_progress() -> bool:
    for name in ("MERGE_HEAD", "REBASE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD"):
        result = subprocess.run(
            ["git", "rev-parse", "--verify", "--quiet", name],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if result.returncode == 0:
            return True
    return False


def read_state(path: Path) -> dict[str, str] | None:
    if not path.exists():
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise RuntimeError(f"无法读取恢复状态 {path}: {exc}") from exc
    if not isinstance(value, dict) or not isinstance(value.get("original_branch"), str):
        raise RuntimeError(f"恢复状态格式无效：{path}")
    return value


def write_state(path: Path, original_branch: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    value = {
        "version": 1,
        "original_branch": original_branch,
        "remote": REMOTE,
        "main_branch": MAIN_BRANCH,
        "dev_branch": DEV_BRANCH,
    }
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def clear_state(path: Path) -> None:
    try:
        path.unlink()
    except FileNotFoundError:
        pass


def restore_branch(original_branch: str) -> bool:
    if current_branch() == original_branch:
        return True
    if operation_in_progress() or worktree_dirty():
        print("[git-upd-dev] 当前 Git 状态不安全，无法切回原分支。")
        return False
    try:
        run_git("switch", original_branch)
    except GitError as exc:
        print(f"[git-upd-dev] 切回原分支失败：{exc}")
        return False
    return True


def print_conflict(state: Path) -> None:
    print("[git-upd-dev] 合并 main 到 dev 时发生冲突。")
    print("[git-upd-dev] 保持 dev 的 merge 状态，使用 $resolving-merge-conflicts 解决并提交。")
    print("[git-upd-dev] 解决完成后重新运行：")
    print("  python -X utf8 .agents/skills/git-upd-dev/scripts/update_dev.py")
    print(f"[git-upd-dev] 恢复状态：{state}")


def resume_from_state(state: dict[str, str]) -> str:
    expected = {
        "remote": REMOTE,
        "main_branch": MAIN_BRANCH,
        "dev_branch": DEV_BRANCH,
    }
    for key, value in expected.items():
        if state.get(key, value) != value:
            raise RuntimeError(f"恢复状态与当前脚本配置不一致：{key}")

    original_branch = state["original_branch"]
    branch = current_branch()
    if branch != DEV_BRANCH:
        raise RuntimeError(
            f"检测到未完成的 upd dev 恢复状态；当前分支为 {branch}，请先切回 {DEV_BRANCH}"
        )
    if operation_in_progress():
        raise ConflictPendingError("冲突尚未提交，请先使用 $resolving-merge-conflicts 完成 merge")
    if worktree_dirty():
        raise ConflictPendingError("冲突已修改但尚未提交，请先完成并提交解决结果")
    return original_branch


def main(argv: Sequence[str] | None = None) -> int:
    arguments = list(sys.argv[1:] if argv is None else argv)
    if arguments:
        if arguments in (["-h"], ["--help"]):
            print(USAGE)
            return 0
        print(f"[git-upd-dev] 不接受参数。用法：{USAGE}")
        return ERROR_EXIT

    original_branch: str | None = None
    state: Path | None = None
    on_dev = False
    keep_dev = False
    exit_code = ERROR_EXIT

    try:
        run_git("rev-parse", "--show-toplevel")
        state = state_path()
        saved_state = read_state(state)

        if saved_state is not None:
            if current_branch() == DEV_BRANCH:
                on_dev = True
                keep_dev = operation_in_progress() or worktree_dirty()
            original_branch = resume_from_state(saved_state)
            print("[git-upd-dev] 继续已解决的 dev merge，跳过重新 fetch/merge。")
        else:
            original_branch = current_branch()
            if worktree_dirty():
                raise RuntimeError("工作区或 index 不干净；请先处理现有改动")

            run_git("switch", DEV_BRANCH)
            on_dev = True

            # Refresh dev before merging main so the eventual push includes the
            # latest remote dev history. Refuse divergence rather than creating
            # an implicit merge commit during the pull.
            run_git("pull", "--ff-only", REMOTE, DEV_BRANCH)

            if local_branch_exists(MAIN_BRANCH):
                run_git("switch", MAIN_BRANCH)
                run_git("pull", "--ff-only", REMOTE, MAIN_BRANCH)
                run_git("switch", DEV_BRANCH)
            else:
                run_git("fetch", REMOTE, MAIN_BRANCH)
            merge_target = f"{REMOTE}/{MAIN_BRANCH}"

            run_git("rev-parse", "--verify", f"refs/heads/{DEV_BRANCH}")
            run_git("rev-parse", "--verify", f"refs/remotes/{REMOTE}/{MAIN_BRANCH}")

            write_state(state, original_branch)

            try:
                run_git("merge", "--no-edit", merge_target)
            except GitError as exc:
                if operation_in_progress():
                    print_conflict(state)
                    keep_dev = True
                    exit_code = CONFLICT_EXIT
                    return exit_code
                raise exc

        push_error: GitError | None = None
        try:
            run_git("push", REMOTE, DEV_BRANCH)
        except GitError as exc:
            push_error = exc

        if push_error is not None:
            print(f"[git-upd-dev] push dev 失败：{push_error}")
        else:
            print(f"[git-upd-dev] 已将最新 {MAIN_BRANCH} 合并到 {DEV_BRANCH} 并推送。")
            exit_code = 0
    except GitError as exc:
        print(f"[git-upd-dev] Git 操作失败：{exc}")
    except ConflictPendingError as exc:
        print(f"[git-upd-dev] {exc}")
        print("[git-upd-dev] 保持 dev 状态，使用 $resolving-merge-conflicts 完成后再重试。")
        keep_dev = True
        exit_code = CONFLICT_EXIT
    except RuntimeError as exc:
        print(f"[git-upd-dev] 已停止：{exc}")
    except OSError as exc:
        print(f"[git-upd-dev] 文件操作失败：{exc}")
    finally:
        if on_dev and original_branch is not None and not keep_dev:
            if restore_branch(original_branch):
                if state is not None:
                    try:
                        clear_state(state)
                    except OSError as exc:
                        print(f"[git-upd-dev] 无法清理恢复状态：{exc}")
                        exit_code = ERROR_EXIT
                if exit_code == 0:
                    print(
                        f"[git-upd-dev] 当前分支：{current_branch()}（原分支：{original_branch}）"
                    )
            else:
                exit_code = ERROR_EXIT

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
