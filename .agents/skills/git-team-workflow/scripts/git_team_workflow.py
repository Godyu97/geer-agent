#!/usr/bin/env python3
"""Run the repository Git team workflow with resumable conflict handling."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import unicodedata
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable


EXIT_OK = 0
EXIT_ERROR = 1
EXIT_CONFLICT = 3
STATE_VERSION = 1
COMMIT_SUBJECT_RE = re.compile(
    r"^(feat|fix|refactor|docs|test|perf|build|ci|chore|revert): "
    r"\([a-z0-9]+(?:-[a-z0-9]+)*\).+$"
)
TRAILING_PUNCTUATION = ".,;:!?，。；：！？"


def normalize_git_path(path: str) -> str:
    """Normalize Git path output for reliable cross-platform scope matching."""

    return unicodedata.normalize("NFC", path.replace("\\", "/"))


class WorkflowError(RuntimeError):
    """A safe, actionable workflow failure."""

    def __init__(self, message: str, context: dict[str, Any] | None = None) -> None:
        super().__init__(message)
        self.context = context or {}


class ConflictDetected(WorkflowError):
    """A Git operation left an in-progress merge/rebase conflict."""

    def __init__(self, stage: str, detail: str) -> None:
        super().__init__(detail)
        self.stage = stage


class Git:
    def __init__(self, repo: Path) -> None:
        self.repo = repo

    def raw(self, args: Iterable[str]) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["git", *[str(arg) for arg in args]],
            cwd=self.repo,
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=True,
        )

    def run(self, args: Iterable[str], stage: str) -> subprocess.CompletedProcess[str]:
        command = [str(arg) for arg in args]
        print(f"[git-team-workflow] git {subprocess.list2cmdline(command)}")
        result = self.raw(command)
        if result.stdout.strip():
            print(result.stdout.rstrip())
        if result.stderr.strip():
            print(result.stderr.rstrip(), file=sys.stderr)
        if result.returncode == 0:
            return result
        if self.has_conflict():
            raise ConflictDetected(stage, f"Git 命令返回 {result.returncode}")
        detail = result.stderr.strip() or result.stdout.strip() or "无错误输出"
        raise WorkflowError(
            f"阶段 {stage} 执行失败（退出码 {result.returncode}）：{detail}"
        )

    def git_dir(self) -> Path:
        result = self.raw(["rev-parse", "--git-dir"])
        if result.returncode != 0:
            raise WorkflowError("当前目录不是 Git 仓库，无法定位 .git 目录")
        path = Path(result.stdout.strip())
        if not path.is_absolute():
            path = self.repo / path
        return path.resolve()

    def git_path(self, name: str) -> Path:
        result = self.raw(["rev-parse", "--git-path", name])
        if result.returncode != 0:
            return self.git_dir() / name
        path = Path(result.stdout.strip())
        if not path.is_absolute():
            path = self.repo / path
        return path.resolve()

    def state_path(self) -> Path:
        return self.git_dir() / "git-team-workflow-state.json"

    def has_conflict(self) -> bool:
        unmerged = self.raw(["ls-files", "-u"])
        if unmerged.returncode == 0 and unmerged.stdout.strip():
            return True
        return any(
            self.git_path(marker).exists()
            for marker in ("MERGE_HEAD", "REBASE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD")
        )

    def conflict_paths(self) -> list[str]:
        result = self.raw(["ls-files", "-u", "-z"])
        paths: list[str] = []
        for record in result.stdout.split("\0"):
            if "\t" in record:
                path = normalize_git_path(record.split("\t", 1)[1])
                if path not in paths:
                    paths.append(path)
        return paths

    def current_branch(self) -> str | None:
        result = self.raw(["symbolic-ref", "--quiet", "--short", "HEAD"])
        if result.returncode != 0:
            return None
        return result.stdout.strip() or None

    def head(self) -> str:
        result = self.raw(["rev-parse", "HEAD"])
        if result.returncode != 0:
            raise WorkflowError("无法读取当前 Git HEAD")
        return result.stdout.strip()

    def ref_oid(self, ref: str) -> str | None:
        result = self.raw(["rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}"])
        if result.returncode != 0:
            return None
        return result.stdout.strip() or None

    def is_ancestor(self, ancestor: str, descendant: str) -> bool:
        result = self.raw(["merge-base", "--is-ancestor", ancestor, descendant])
        if result.returncode == 0:
            return True
        if result.returncode == 1:
            return False
        detail = result.stderr.strip() or result.stdout.strip() or "无法判断提交关系"
        raise WorkflowError(detail)

    def status_lines(self) -> list[str]:
        result = self.raw(["status", "--porcelain=v1", "--untracked-files=all"])
        if result.returncode != 0:
            detail = result.stderr.strip() or "无法读取工作区状态"
            raise WorkflowError(detail)
        return [line for line in result.stdout.splitlines() if line]

    def changed_paths(self) -> list[str]:
        commands = (
            ["diff", "--name-only"],
            ["diff", "--cached", "--name-only"],
            ["ls-files", "--others", "--exclude-standard"],
        )
        paths: set[str] = set()
        for command in commands:
            result = self.raw([*command, "-z"])
            if result.returncode != 0:
                detail = result.stderr.strip() or "无法读取变更文件"
                raise WorkflowError(detail)
            paths.update(
                normalize_git_path(path)
                for path in result.stdout.split("\0")
                if path
            )
        return sorted(paths)

    def cached_paths(self) -> list[str]:
        result = self.raw(["diff", "--cached", "--name-only", "-z"])
        if result.returncode != 0:
            detail = result.stderr.strip() or "无法读取暂存区"
            raise WorkflowError(detail)
        return sorted(
            normalize_git_path(path)
            for path in result.stdout.split("\0")
            if path
        )


class StateStore:
    def __init__(self, path: Path) -> None:
        self.path = path

    def load(self) -> dict[str, Any] | None:
        if not self.path.exists():
            return None
        try:
            state = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            raise WorkflowError(f"无法读取 workflow 状态文件 {self.path}: {exc}") from exc
        if not isinstance(state, dict) or state.get("version") != STATE_VERSION:
            raise WorkflowError(f"workflow 状态文件版本不受支持：{self.path}")
        return state

    def save(self, state: dict[str, Any]) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        fd, temp_name = tempfile.mkstemp(
            prefix="git-team-workflow-", suffix=".tmp", dir=self.path.parent
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as handle:
                json.dump(state, handle, ensure_ascii=False, indent=2)
                handle.write("\n")
                handle.flush()
                os.fsync(handle.fileno())
            os.replace(temp_name, self.path)
        finally:
            if os.path.exists(temp_name):
                os.unlink(temp_name)

    def clear(self) -> None:
        try:
            self.path.unlink()
        except FileNotFoundError:
            pass
        except OSError as exc:
            raise WorkflowError(f"流程完成，但无法清理状态文件 {self.path}: {exc}") from exc


def resolve_repo(repo_arg: str | None) -> Path:
    start = Path(repo_arg or os.getcwd()).expanduser().resolve()
    result = subprocess.run(
        ["git", "-C", str(start), "rev-parse", "--show-toplevel"],
        text=True,
        encoding="utf-8",
        errors="replace",
        capture_output=True,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or "当前目录不是 Git 仓库"
        raise WorkflowError(detail)
    return Path(result.stdout.strip()).resolve()


def add_common_options(parser: argparse.ArgumentParser, suppress: bool = False) -> None:
    default = argparse.SUPPRESS if suppress else None
    parser.add_argument("--repo", default=default, help="Git 仓库路径，默认使用当前目录")
    parser.add_argument("--remote", default=default, help="远端名，默认 origin")
    parser.add_argument("--main", default=default, help="主分支名，默认 main")
    parser.add_argument("--test", default=default, help="测试分支名，默认 test")
    parser.add_argument(
        "--resume",
        action="store_true",
        default=argparse.SUPPRESS if suppress else False,
        help="显式要求从已有状态继续；默认检测到状态时也会继续",
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run the git-team-workflow begin/end flow with resumable conflict handling."
    )
    add_common_options(parser)
    subparsers = parser.add_subparsers(dest="command", required=True)

    begin = subparsers.add_parser(
        "begin",
        aliases=["coding-begin", "codeing-begin"],
        help="同步 feature <- remote/main 并推送 feature",
    )
    add_common_options(begin, suppress=True)

    end = subparsers.add_parser(
        "end",
        aliases=["coding-end", "codeing-end"],
        help="提交 feature，合入 main，晋级 test 并推送",
    )
    add_common_options(end, suppress=True)
    commit_group = end.add_mutually_exclusive_group()
    commit_group.add_argument(
        "--all",
        dest="add_all",
        action="store_true",
        help="确认全部工作区变更属于当前任务后，暂存全部变更",
    )
    commit_group.add_argument(
        "--path",
        dest="paths",
        action="append",
        help="暂存指定文件或目录；可重复传入，不支持 glob",
    )
    end.add_argument(
        "-m",
        "--commit-message",
        help="feature commit subject，例如：feat: (task)增加批量接口",
    )
    end.add_argument(
        "--commit-group",
        dest="commit_groups",
        action="append",
        nargs="+",
        metavar="VALUE",
        help=(
            "一次创建多个 feature commit；每组格式为"
            " --commit-group <message> <path> [<path> ...]，可重复传入"
        ),
    )
    return parser


def resolve_config(args: argparse.Namespace, state: dict[str, Any] | None) -> dict[str, str]:
    defaults = {"remote": "origin", "main": "main", "test": "test"}
    config: dict[str, str] = {}
    for key, default in defaults.items():
        cli_value = getattr(args, key, None)
        if state is not None:
            stored_value = state[key]
            if cli_value is not None and cli_value != stored_value:
                raise WorkflowError(
                    f"继续已有流程时不能修改 --{key}：状态为 {stored_value}，参数为 {cli_value}"
                )
            config[key] = stored_value
        else:
            config[key] = default if cli_value is None else cli_value
        if not config[key] or any(char.isspace() for char in config[key]):
            raise WorkflowError(f"--{key} 不能为空或包含空白字符")
    if config["main"] == config["test"]:
        raise WorkflowError("--main 与 --test 不能相同")
    return config


def new_summary(
    mode: str,
    config: dict[str, str] | None,
    original_branch: str | None,
) -> dict[str, Any]:
    config = config or {}
    return {
        "schema_version": 1,
        "workflow": "git-team-workflow",
        "mode": mode,
        "status": "running",
        "stage": None,
        "exit_code": None,
        "remote": config.get("remote"),
        "branches": {
            "original": original_branch,
            "main": config.get("main"),
            "test": config.get("test"),
            "current": None,
            "final": None,
        },
        "fetched_remote_refs": {},
        "refs": {"local": {}, "remote_tracking": {}},
        "feature": {
            "head_before": None,
            "head_after": None,
            "commits_created": [],
        },
        "scope": {
            "commit_message": None,
            "commit_groups": [],
            "add_all": False,
            "requested_selectors": [],
            "selectors": [],
            "changed_paths": [],
            "outside_paths": [],
        },
        "merges": [],
        "pushes": [],
        "pushed_branches": [],
        "conflicts": [],
        "current_conflict": None,
        "current_blocker": None,
        "partial_remote_state": None,
        "worktree": {"clean": None, "status": []},
        "state_file": None,
    }


def summary_for(state: dict[str, Any]) -> dict[str, Any]:
    summary = state.get("summary")
    if not isinstance(summary, dict):
        summary = new_summary(
            state["mode"],
            {
                "remote": state["remote"],
                "main": state["main"],
                "test": state["test"],
            },
            state["original_branch"],
        )
        state["summary"] = summary
    scope_defaults = new_summary(
        state["mode"],
        {
            "remote": state["remote"],
            "main": state["main"],
            "test": state["test"],
        },
        state["original_branch"],
    )["scope"]
    if not isinstance(summary.get("scope"), dict):
        summary["scope"] = {}
    for key, default in scope_defaults.items():
        summary["scope"].setdefault(key, default)
    return summary


def record_feature_commit(
    state: dict[str, Any],
    git: Git,
    subject: str | None = None,
) -> None:
    summary = summary_for(state)
    oid = git.head()
    if subject is None:
        subject = git.raw(["show", "-s", "--format=%s", "HEAD"]).stdout.strip()
    commits = summary["feature"]["commits_created"]
    if not any(commit.get("oid") == oid for commit in commits):
        commits.append({"oid": oid, "subject": subject})
    summary["feature"]["head_after"] = oid


def classify_merge_action(before_oid: str, source_oid: str, result_oid: str) -> str:
    if before_oid == result_oid:
        return "already_up_to_date"
    if result_oid == source_oid:
        return "fast_forward"
    return "merge_commit"


def record_merge(
    state: dict[str, Any],
    stage: str,
    source_ref: str,
    source_oid: str,
    target_branch: str,
    before_oid: str,
    result_oid: str,
) -> None:
    summary = summary_for(state)
    event = {
        "stage": stage,
        "source_ref": source_ref,
        "source_oid": source_oid,
        "target_branch": target_branch,
        "before_oid": before_oid,
        "result_oid": result_oid,
        "action": classify_merge_action(before_oid, source_oid, result_oid),
    }
    for merge in summary["merges"]:
        if merge.get("stage") == stage and merge.get("result_oid") == result_oid:
            merge.update(event)
            return
    summary["merges"].append(event)


def classify_push_action(before_oid: str | None, result_oid: str) -> str:
    if before_oid is None:
        return "created"
    if before_oid == result_oid:
        return "already_up_to_date"
    return "updated"


def record_push(
    state: dict[str, Any],
    config: dict[str, str],
    stage: str,
    branch: str,
    oid: str,
    before_remote_oid: str | None,
) -> None:
    summary = summary_for(state)
    event = {
        "stage": stage,
        "remote": config["remote"],
        "remote_ref": f"{config['remote']}/{branch}",
        "branch": branch,
        "before_oid": before_remote_oid,
        "oid": oid,
        "action": classify_push_action(before_remote_oid, oid),
    }
    for push in summary["pushes"]:
        if push.get("stage") == stage and push.get("oid") == oid:
            push.update(event)
            break
    else:
        summary["pushes"].append(event)
    summary["pushed_branches"] = [push["branch"] for push in summary["pushes"]]


def format_remote_refs(config: dict[str, str], original_branch: str, refs: dict[str, str]) -> dict[str, str]:
    formatted: dict[str, str] = {}
    for key, oid in refs.items():
        branch = original_branch if key == "feature" else config[key]
        formatted[f"{config['remote']}/{branch}"] = oid
    return formatted


def new_state(mode: str, config: dict[str, str], original_branch: str) -> dict[str, Any]:
    return {
        "version": STATE_VERSION,
        "mode": mode,
        "remote": config["remote"],
        "main": config["main"],
        "test": config["test"],
        "original_branch": original_branch,
        "stage": f"{mode}.fetch" if mode == "begin" else "end.commit",
        "created_at": datetime.now(timezone.utc).isoformat(),
        "summary": new_summary(mode, config, original_branch),
    }


def ensure_remote(git: Git, remote: str) -> None:
    result = git.raw(["remote", "get-url", remote])
    if result.returncode != 0:
        raise WorkflowError(f"远端不存在或不可用：{remote}")


def require_feature_branch(git: Git, config: dict[str, str]) -> str:
    branch = git.current_branch()
    if branch is None:
        raise WorkflowError("当前处于 detached HEAD，必须先切换到命名 feature 分支")
    if branch in (config["main"], config["test"]):
        raise WorkflowError(
            f"当前分支 {branch} 是共享分支；请在 feature 分支执行 workflow"
        )
    return branch


def expect_branch(git: Git, branch: str) -> None:
    current = git.current_branch()
    if current != branch:
        raise WorkflowError(f"当前分支应为 {branch}，实际为 {current or 'detached HEAD'}")


def ensure_clean(git: Git, context: str) -> None:
    lines = git.status_lines()
    if lines:
        preview = "; ".join(lines[:12])
        suffix = " ..." if len(lines) > 12 else ""
        raise WorkflowError(f"{context}要求工作区和暂存区干净，当前变更：{preview}{suffix}")


def safe_ref_oid(git: Git, ref: str) -> str | None:
    try:
        return git.ref_oid(ref)
    except WorkflowError:
        return None


def prepare_summary(
    git: Git | None,
    store: StateStore | None,
    state: dict[str, Any] | None,
    mode: str,
    status: str,
    stage: str | None,
    exit_code: int,
    blocker: dict[str, Any] | None = None,
    conflict: dict[str, Any] | None = None,
    context: dict[str, Any] | None = None,
) -> dict[str, Any]:
    summary = summary_for(state) if state is not None else new_summary(mode, None, None)
    if context:
        if context.get("remote") is not None:
            summary["remote"] = context["remote"]
        branches = context.get("branches")
        if isinstance(branches, dict):
            summary["branches"].update(branches)
        scope = context.get("scope")
        if isinstance(scope, dict):
            summary["scope"].update(scope)
    summary["status"] = status
    summary["stage"] = stage
    summary["exit_code"] = exit_code
    summary["current_blocker"] = blocker
    summary["current_conflict"] = conflict
    if store is not None:
        summary["state_file"] = str(store.path)

    if conflict is not None and not any(
        item.get("stage") == conflict.get("stage")
        and item.get("paths") == conflict.get("paths")
        for item in summary["conflicts"]
    ):
        summary["conflicts"].append(conflict)

    if git is not None:
        try:
            current_branch = git.current_branch()
        except WorkflowError:
            current_branch = None
        summary["branches"]["current"] = current_branch
        if status == "success":
            summary["branches"]["final"] = current_branch

        try:
            status_lines = git.status_lines()
            summary["worktree"] = {
                "clean": not status_lines,
                "status": status_lines,
            }
        except WorkflowError as exc:
            summary["worktree"] = {"clean": None, "status": [], "error": str(exc)}

        remote = summary.get("remote")
        branches = summary["branches"]
        local_refs: dict[str, str | None] = {}
        remote_refs: dict[str, str | None] = {}
        for branch in (branches.get("original"), branches.get("main"), branches.get("test")):
            if not branch:
                continue
            local_refs[branch] = safe_ref_oid(git, f"refs/heads/{branch}")
            if remote:
                remote_refs[f"{remote}/{branch}"] = safe_ref_oid(git, f"{remote}/{branch}")
        summary["refs"] = {"local": local_refs, "remote_tracking": remote_refs}
        original_branch = branches.get("original")
        if original_branch:
            summary["feature"]["head_after"] = local_refs.get(original_branch)

    summary["partial_remote_state"] = (
        None
        if status == "success"
        else {
            "fetched_remote_refs": summary.get("fetched_remote_refs", {}),
            "pushed": summary.get("pushes", []),
        }
    )
    return summary


def emit_summary(summary: dict[str, Any], stream: Any = sys.stdout) -> None:
    print(
        "[git-team-workflow] SUMMARY_JSON "
        + json.dumps(summary, ensure_ascii=True, sort_keys=True, separators=(",", ":")),
        file=stream,
    )


def persist_summary(store: StateStore | None, state: dict[str, Any] | None) -> None:
    if store is None or state is None:
        return
    try:
        store.save(state)
    except WorkflowError as exc:
        print(f"[git-team-workflow] 无法持久化结构化摘要：{exc}", file=sys.stderr)


def capture_remote_refs(
    git: Git,
    config: dict[str, str],
    include_feature: str | None,
    require_test: bool,
) -> dict[str, str]:
    refs: dict[str, str] = {}
    required = {"main": f"{config['remote']}/{config['main']}"}
    if require_test:
        required["test"] = f"{config['remote']}/{config['test']}"
    for key, ref in required.items():
        oid = git.ref_oid(ref)
        if oid is None:
            raise WorkflowError(f"远端分支不存在：{ref}")
        refs[key] = oid
    if include_feature is not None:
        ref = f"{config['remote']}/{include_feature}"
        oid = git.ref_oid(ref)
        if oid is not None:
            refs["feature"] = oid
    return refs


def ensure_snapshot(git: Git, config: dict[str, str], key: str, expected: str) -> None:
    ref_name = config["remote"] + "/" + (config["main"] if key == "main" else config["test"])
    actual = git.ref_oid(ref_name)
    if actual != expected:
        raise WorkflowError(
            f"远端快照已变化，拒绝继续：{ref_name} 预期 {expected[:12]}，实际 {actual or '不存在'}"
        )


def advance(store: StateStore, state: dict[str, Any], next_stage: str) -> None:
    state["stage"] = next_stage
    state.pop("merge_head_before", None)
    state.pop("merge_source_oid", None)
    store.save(state)


def run_merge_stage(
    git: Git,
    store: StateStore,
    state: dict[str, Any],
    source_ref: str,
    stage: str,
    next_stage: str,
    expected_branch: str,
) -> None:
    expect_branch(git, expected_branch)
    if git.has_conflict():
        raise ConflictDetected(stage, "阶段开始时已经存在未完成的 Git 合并/变基")
    ensure_clean(git, f"{stage} 合并前")

    before = state.get("merge_head_before")
    source_oid = state.get("merge_source_oid")
    if before is not None or source_oid is not None:
        if not before or not source_oid:
            raise WorkflowError(f"状态文件中的 {stage} 合并记录不完整")
        current = git.head()
        if current == before:
            raise WorkflowError(
                f"{stage} 尚未完成；请不要跳过合并，检查冲突处理结果后再重试"
            )
        if not git.is_ancestor(source_oid, current):
            raise WorkflowError(
                f"无法确认 {stage} 已合入记录的源提交 {source_oid[:12]}，停止以避免跳过合并"
            )
        record_merge(state, stage, source_ref, source_oid, expected_branch, before, current)
        print(f"[git-team-workflow] 检测到 {stage} 已完成，继续下一阶段")
        advance(store, state, next_stage)
        return

    source_oid = git.ref_oid(source_ref)
    if source_oid is None:
        raise WorkflowError(f"合并源不存在：{source_ref}")
    state["merge_head_before"] = git.head()
    state["merge_source_oid"] = source_oid
    store.save(state)
    git.run(["merge", "--no-edit", source_ref], stage)
    record_merge(state, stage, source_ref, source_oid, expected_branch, state["merge_head_before"], git.head())
    advance(store, state, next_stage)


def sync_feature(git: Git, config: dict[str, str], original_branch: str, expected: str) -> None:
    ref = f"{config['remote']}/{original_branch}"
    actual = git.ref_oid(ref)
    if actual != expected:
        raise WorkflowError(
            f"远端快照已变化，拒绝继续：{ref} 预期 {expected[:12]}，实际 {actual or '不存在'}"
        )
    local = git.head()
    if local == expected:
        return
    if git.is_ancestor(expected, local):
        print(f"[git-team-workflow] 本地 feature 已领先 {ref}，保留本地提交")
        return
    if not git.is_ancestor(local, expected):
        raise WorkflowError(f"feature 分支与 {ref} 已分叉，拒绝自动合并")
    git.run(["merge", "--ff-only", ref], "begin.sync-feature")


def switch_or_create(git: Git, branch: str, tracking_ref: str, stage: str) -> None:
    if git.current_branch() == branch:
        return
    if git.ref_oid(f"refs/heads/{branch}") is not None:
        git.run(["switch", branch], stage)
        return
    git.run(["switch", "-c", branch, "--track", tracking_ref], stage)


def sync_shared_branch(git: Git, branch: str, remote_ref: str, expected: str, stage: str) -> None:
    actual_remote = git.ref_oid(remote_ref)
    if actual_remote != expected:
        raise WorkflowError(
            f"远端快照已变化，拒绝继续：{remote_ref} 预期 {expected[:12]}，实际 {actual_remote or '不存在'}"
        )
    local = git.head()
    if local == expected:
        return
    if git.is_ancestor(expected, local):
        raise WorkflowError(f"本地共享分支 {branch} 含有远端不存在的提交，拒绝继续")
    if not git.is_ancestor(local, expected):
        raise WorkflowError(f"本地共享分支 {branch} 与 {remote_ref} 已分叉，拒绝继续")
    git.run(["merge", "--ff-only", remote_ref], stage)


def normalize_selector(raw: str, repo: Path) -> str:
    if not raw or any(char in raw for char in "*?["):
        raise WorkflowError(f"--path 必须是非空文件/目录路径且不支持 glob：{raw!r}")
    path = Path(raw).expanduser()
    if path.is_absolute():
        try:
            relative = os.path.relpath(str(path.resolve()), str(repo))
        except ValueError as exc:
            raise WorkflowError(f"--path 不在仓库内：{raw}") from exc
        if relative == ".." or relative.startswith(".." + os.sep):
            raise WorkflowError(f"--path 不在仓库内：{raw}")
    else:
        relative = raw
    normalized = normalize_git_path(relative)
    while normalized.startswith("./"):
        normalized = normalized[2:]
    normalized = normalized.rstrip("/")
    if not normalized or normalized == "." or normalized.startswith("../"):
        raise WorkflowError(f"--path 不在仓库内：{raw}")
    return normalized


def path_matches(path: str, selectors: list[str]) -> bool:
    normalized = normalize_git_path(path)
    while normalized.startswith("./"):
        normalized = normalized[2:]
    return any(normalized == selector or normalized.startswith(selector + "/") for selector in selectors)


def validate_commit_subject(message: str) -> None:
    if "\n" in message or "\r" in message:
        raise WorkflowError("commit subject 必须是单行")
    if message != message.strip():
        raise WorkflowError("commit subject 不能有首尾空格")
    if len(message) > 72:
        raise WorkflowError("commit subject 最多 72 个字符")
    if not COMMIT_SUBJECT_RE.fullmatch(message):
        raise WorkflowError(
            "commit subject 不符合格式：<type>: (<lower-kebab-scope>)<summary>"
        )
    if message[-1] in TRAILING_PUNCTUATION:
        raise WorkflowError("commit subject 不能以标点符号结尾")


def validate_change_scope(
    git: Git,
    selectors: list[str],
    context: dict[str, Any] | None = None,
    changed: list[str] | None = None,
) -> None:
    changed = git.changed_paths() if changed is None else list(changed)
    outside = [path for path in changed if not path_matches(path, selectors)]
    if context is not None:
        scope = context.setdefault("scope", {})
        scope.update(
            {
                "selectors": list(selectors),
                "changed_paths": changed,
                "outside_paths": outside,
            }
        )
    if outside:
        preview = ", ".join(outside[:12])
        suffix = " ..." if len(outside) > 12 else ""
        raise WorkflowError(
            f"检测到未被 --path 覆盖的其他变更，拒绝部分提交：{preview}{suffix}"
        )


def normalize_commit_groups(
    raw_groups: list[list[str]], repo: Path
) -> list[dict[str, Any]]:
    groups: list[dict[str, Any]] = []
    for index, raw_group in enumerate(raw_groups, start=1):
        if len(raw_group) < 2:
            raise WorkflowError(
                f"第 {index} 个 --commit-group 必须包含 commit subject 和至少一个 --path"
            )
        message = raw_group[0]
        validate_commit_subject(message)
        selectors: list[str] = []
        for raw_selector in raw_group[1:]:
            selector = normalize_selector(raw_selector, repo)
            if selector not in selectors:
                selectors.append(selector)
        groups.append(
            {
                "commit_message": message,
                "requested_selectors": list(raw_group[1:]),
                "selectors": selectors,
            }
        )
    return groups


def validate_commit_group_scope(
    git: Git,
    groups: list[dict[str, Any]],
    changed: list[str],
    context: dict[str, Any] | None = None,
) -> None:
    matched_paths: list[list[str]] = [[] for _ in groups]
    outside: list[str] = []
    overlaps: list[str] = []
    for path in changed:
        matches = [
            index
            for index, group in enumerate(groups)
            if path_matches(path, group["selectors"])
        ]
        if not matches:
            outside.append(path)
        elif len(matches) > 1:
            overlaps.append(path)
        else:
            matched_paths[matches[0]].append(path)

    if context is not None:
        scope = context.setdefault("scope", {})
        scope["commit_groups"] = [
            {
                "commit_message": group["commit_message"],
                "requested_selectors": group["requested_selectors"],
                "selectors": group["selectors"],
                "changed_paths": matched_paths[index],
            }
            for index, group in enumerate(groups)
        ]
        scope["outside_paths"] = outside

    if outside:
        preview = ", ".join(outside[:12])
        suffix = " ..." if len(outside) > 12 else ""
        raise WorkflowError(
            f"检测到未被任何 --commit-group 覆盖的变更，拒绝多提交：{preview}{suffix}"
        )
    if overlaps:
        preview = ", ".join(overlaps[:12])
        suffix = " ..." if len(overlaps) > 12 else ""
        raise WorkflowError(
            f"检测到同时属于多个 --commit-group 的变更，无法安全分组：{preview}{suffix}"
        )
    empty = [
        str(index + 1)
        for index, paths in enumerate(matched_paths)
        if not paths
    ]
    if empty:
        raise WorkflowError(
            f"第 {', '.join(empty)} 个 --commit-group 没有匹配到当前变更，拒绝创建空 commit"
        )


def validate_resume_commit_args(args: argparse.Namespace, state: dict[str, Any]) -> None:
    message = getattr(args, "commit_message", None)
    if message is not None and message != state.get("commit_message"):
        raise WorkflowError("继续已有流程时不能修改 --commit-message")
    supplied_paths = getattr(args, "paths", None)
    if supplied_paths is not None and supplied_paths != state.get("selectors"):
        raise WorkflowError("继续已有流程时不能修改 --path")
    if getattr(args, "add_all", False) and not state.get("add_all", False):
        raise WorkflowError("继续已有流程时不能把暂存范围改为 --all")
    supplied_groups = getattr(args, "commit_groups", None)
    if (
        supplied_groups is not None
        and supplied_groups != state.get("commit_groups_requested")
    ):
        raise WorkflowError("继续已有流程时不能修改 --commit-group")


def run_begin(git: Git, store: StateStore, args: argparse.Namespace) -> None:
    state = store.load()
    if state is not None and state.get("mode") != "begin":
        raise WorkflowError(f"已有未完成的 {state.get('mode')} 流程，请先完成它")
    config = resolve_config(args, state)

    if state is None:
        ensure_remote(git, config["remote"])
        original_branch = require_feature_branch(git, config)
        ensure_clean(git, "coding begin")
        state = new_state("begin", config, original_branch)
        store.save(state)
    else:
        original_branch = state["original_branch"]

    summary = summary_for(state)
    if summary["feature"]["head_before"] is None:
        summary["feature"]["head_before"] = git.head()
        store.save(state)

    while True:
        stage = state["stage"]
        if stage == "begin.fetch":
            expect_branch(git, original_branch)
            ensure_clean(git, "coding begin fetch 前")
            git.run(["fetch", config["remote"], "--prune"], stage)
            state["remote_refs"] = capture_remote_refs(
                git, config, original_branch, require_test=False
            )
            summary_for(state)["fetched_remote_refs"] = format_remote_refs(
                config, original_branch, state["remote_refs"]
            )
            next_stage = "begin.sync-feature" if "feature" in state["remote_refs"] else "begin.merge-main"
            advance(store, state, next_stage)
        elif stage == "begin.sync-feature":
            expect_branch(git, original_branch)
            ensure_clean(git, "同步 feature 前")
            sync_feature(git, config, original_branch, state["remote_refs"]["feature"])
            advance(store, state, "begin.merge-main")
        elif stage == "begin.merge-main":
            ensure_snapshot(git, config, "main", state["remote_refs"]["main"])
            run_merge_stage(
                git,
                store,
                state,
                f"{config['remote']}/{config['main']}",
                stage,
                "begin.push-feature",
                original_branch,
            )
        elif stage == "begin.push-feature":
            expect_branch(git, original_branch)
            ensure_clean(git, "推送 feature 前")
            remote_before_oid = safe_ref_oid(
                git, f"{config['remote']}/{original_branch}"
            )
            git.run(
                ["push", "-u", config["remote"], original_branch],
                stage,
            )
            record_push(
                state,
                config,
                stage,
                original_branch,
                git.head(),
                remote_before_oid,
            )
            advance(store, state, "begin.done")
        elif stage == "begin.done":
            expect_branch(git, original_branch)
            ensure_clean(git, "coding begin 完成检查")
            summary = prepare_summary(
                git, store, state, "begin", "success", stage, EXIT_OK
            )
            store.clear()
            print(f"[git-team-workflow] coding begin 完成，当前分支：{original_branch}")
            emit_summary(summary)
            return
        else:
            raise WorkflowError(f"未知 begin 阶段：{stage}")


def initialize_end_state(git: Git, store: StateStore, args: argparse.Namespace, config: dict[str, str]) -> dict[str, Any]:
    message = getattr(args, "commit_message", None)
    supplied_paths = list(getattr(args, "paths", None) or [])
    add_all = bool(getattr(args, "add_all", False))
    raw_commit_groups = getattr(args, "commit_groups", None)
    if raw_commit_groups is not None:
        raw_commit_groups = [list(group) for group in raw_commit_groups]
    selectors: list[str] = []
    commit_groups: list[dict[str, Any]] = []
    preflight_context: dict[str, Any] = {
        "remote": config["remote"],
        "branches": {
            "original": None,
            "main": config["main"],
            "test": config["test"],
            "current": git.current_branch(),
        },
        "scope": {
            "commit_message": message,
            "commit_groups": [],
            "add_all": add_all,
            "requested_selectors": supplied_paths,
            "selectors": [],
            "changed_paths": [],
            "outside_paths": [],
        },
    }
    try:
        original_branch = require_feature_branch(git, config)
        preflight_context["branches"]["original"] = original_branch
        preflight_context["branches"]["current"] = original_branch
        ensure_remote(git, config["remote"])
        changed = git.status_lines()
        commit_needed = bool(changed)
        changed_paths = git.changed_paths() if commit_needed else []
        preflight_context["scope"]["changed_paths"] = changed_paths
        if raw_commit_groups is not None:
            if message or supplied_paths or add_all:
                raise WorkflowError(
                    "--commit-group 不能与 --commit-message、--path 或 --all 同时使用"
                )
            commit_groups = normalize_commit_groups(raw_commit_groups, git.repo)
        if commit_needed:
            if raw_commit_groups is not None:
                validate_commit_group_scope(
                    git,
                    commit_groups,
                    changed_paths,
                    context=preflight_context,
                )
            else:
                if not message:
                    raise WorkflowError("工作区有变更时必须传入 --commit-message")
                validate_commit_subject(message)
                if not add_all:
                    if not supplied_paths:
                        raise WorkflowError(
                            "请用 --path 指定提交范围，或确认全部变更属于任务后使用 --all"
                        )
                    selectors = [normalize_selector(path, git.repo) for path in supplied_paths]
                    validate_change_scope(
                        git,
                        selectors,
                        context=preflight_context,
                        changed=changed_paths,
                    )
        elif message:
            validate_commit_subject(message)
    except WorkflowError as exc:
        if not exc.context:
            exc.context = preflight_context
        raise

    state = new_state("end", config, original_branch)
    state.update(
        {
            "commit_needed": commit_needed,
            "commit_message": message,
            "add_all": add_all,
            "selectors": selectors,
            "commit_groups": commit_groups,
            "commit_groups_requested": raw_commit_groups,
            "commit_index": 0,
        }
    )
    if commit_needed:
        state["commit_head_before"] = git.head()
    summary_for(state)["feature"]["head_before"] = git.head()
    summary_for(state)["scope"].update(preflight_context["scope"])
    store.save(state)
    return state


def run_grouped_feature_commits(
    git: Git, store: StateStore, state: dict[str, Any]
) -> None:
    groups = state.get("commit_groups")
    if not isinstance(groups, list) or not groups:
        raise WorkflowError("多提交状态缺少 commit groups")

    index = int(state.get("commit_index", 0))
    while index < len(groups):
        group = groups[index]
        if not isinstance(group, dict):
            raise WorkflowError(f"第 {index + 1} 个 commit group 状态无效")
        message = group.get("commit_message")
        selectors = group.get("selectors")
        if not isinstance(message, str) or not isinstance(selectors, list) or not selectors:
            raise WorkflowError(f"第 {index + 1} 个 commit group 状态不完整")

        expected_head = state.get("commit_head_before")
        current_head = git.head()
        if expected_head and current_head != expected_head:
            subject = git.raw(["show", "-s", "--format=%s", "HEAD"]).stdout.strip()
            if subject != message:
                raise WorkflowError(
                    f"第 {index + 1} 个 feature commit 已改变，但 HEAD subject 与状态不一致"
                )
            record_feature_commit(state, git, subject)
            index += 1
            state["commit_index"] = index
            state["commit_head_before"] = current_head
            store.save(state)
            print(
                f"[git-team-workflow] 检测到第 {index} 个 feature commit 已完成，继续下一组"
            )
            continue

        changed_paths = git.changed_paths()
        remaining_selectors = [
            selector
            for remaining_group in groups[index:]
            for selector in remaining_group.get("selectors", [])
        ]
        outside = [
            path for path in changed_paths if not path_matches(path, remaining_selectors)
        ]
        if outside:
            preview = ", ".join(outside[:12])
            suffix = " ..." if len(outside) > 12 else ""
            raise WorkflowError(
                f"多提交过程中检测到未被剩余 --commit-group 覆盖的变更：{preview}{suffix}"
            )

        current_group_paths = [
            path for path in changed_paths if path_matches(path, selectors)
        ]
        if not current_group_paths:
            raise WorkflowError(
                f"第 {index + 1} 个 commit group 没有可提交的变更，拒绝创建空 commit"
            )

        cached_outside = [
            path for path in git.cached_paths() if not path_matches(path, selectors)
        ]
        if cached_outside:
            preview = ", ".join(cached_outside[:12])
            suffix = " ..." if len(cached_outside) > 12 else ""
            raise WorkflowError(
                f"暂存区包含后续 commit group 的变更，无法安全创建第 {index + 1} 个 commit：{preview}{suffix}"
            )

        stage = f"end.commit[{index + 1}/{len(groups)}]"
        git.run(["add", "--", *selectors], stage)
        cached = git.cached_paths()
        cached_outside = [
            path for path in cached if not path_matches(path, selectors)
        ]
        if cached_outside:
            preview = ", ".join(cached_outside[:12])
            suffix = " ..." if len(cached_outside) > 12 else ""
            raise WorkflowError(
                f"第 {index + 1} 个 commit group 暂存范围超出声明路径：{preview}{suffix}"
            )
        if not cached:
            raise WorkflowError(
                f"第 {index + 1} 个 commit group 暂存区为空，无法创建 feature commit"
            )

        git.run(["commit", "-m", message], stage)
        record_feature_commit(state, git, message)
        index += 1
        state["commit_index"] = index
        state["commit_head_before"] = git.head()
        store.save(state)

    ensure_clean(git, "多 feature commit 后")


def run_end(git: Git, store: StateStore, args: argparse.Namespace) -> None:
    state = store.load()
    if state is not None and state.get("mode") != "end":
        raise WorkflowError(f"已有未完成的 {state.get('mode')} 流程，请先完成它")
    config = resolve_config(args, state)
    if state is not None:
        validate_resume_commit_args(args, state)
    else:
        state = initialize_end_state(git, store, args, config)

    original_branch = state["original_branch"]
    main_branch = config["main"]
    test_branch = config["test"]
    remote = config["remote"]
    summary = summary_for(state)
    if summary["feature"]["head_before"] is None:
        summary["feature"]["head_before"] = git.head()
        store.save(state)

    while True:
        stage = state["stage"]
        if stage == "end.commit":
            expect_branch(git, original_branch)
            if state["commit_needed"]:
                if state.get("commit_groups"):
                    run_grouped_feature_commits(git, store, state)
                elif not git.status_lines():
                    if git.head() != state["commit_head_before"]:
                        subject = git.raw(["show", "-s", "--format=%s", "HEAD"]).stdout.strip()
                        if subject != state["commit_message"]:
                            raise WorkflowError("工作区已干净但 HEAD subject 与状态中的 commit subject 不一致")
                        record_feature_commit(state, git, subject)
                        print("[git-team-workflow] 检测到 feature commit 已完成，继续下一阶段")
                    else:
                        raise WorkflowError("需要创建 feature commit，但当前 HEAD 未变化")
                else:
                    if not state["add_all"]:
                        validate_change_scope(git, state["selectors"])
                    if state["add_all"]:
                        git.run(["add", "-A"], stage)
                    else:
                        git.run(["add", "--", *state["selectors"]], stage)
                    if not git.cached_paths():
                        raise WorkflowError("暂存区为空，无法创建 feature commit")
                    git.run(["commit", "-m", state["commit_message"]], stage)
                    record_feature_commit(state, git, state["commit_message"])
                ensure_clean(git, "feature commit 后")
            else:
                ensure_clean(git, "无 feature commit 时")
            advance(store, state, "end.fetch")
        elif stage == "end.fetch":
            expect_branch(git, original_branch)
            ensure_clean(git, "coding end fetch 前")
            ensure_remote(git, remote)
            git.run(["fetch", remote, "--prune"], stage)
            state["remote_refs"] = capture_remote_refs(
                git, config, None, require_test=True
            )
            summary_for(state)["fetched_remote_refs"] = format_remote_refs(
                config, original_branch, state["remote_refs"]
            )
            advance(store, state, "end.merge-main-into-feature")
        elif stage == "end.merge-main-into-feature":
            ensure_snapshot(git, config, "main", state["remote_refs"]["main"])
            run_merge_stage(
                git,
                store,
                state,
                f"{remote}/{main_branch}",
                stage,
                "end.push-feature",
                original_branch,
            )
        elif stage == "end.push-feature":
            expect_branch(git, original_branch)
            ensure_clean(git, "推送 feature 前")
            remote_before_oid = safe_ref_oid(git, f"{remote}/{original_branch}")
            git.run(["push", "-u", remote, original_branch], stage)
            record_push(
                state,
                config,
                stage,
                original_branch,
                git.head(),
                remote_before_oid,
            )
            advance(store, state, "end.sync-main")
        elif stage == "end.sync-main":
            current = git.current_branch()
            if current not in (original_branch, main_branch):
                raise WorkflowError(f"同步 main 阶段当前分支异常：{current or 'detached HEAD'}")
            ensure_clean(git, "切换 main 前")
            switch_or_create(git, main_branch, f"{remote}/{main_branch}", stage)
            ensure_clean(git, "同步 main 前")
            sync_shared_branch(
                git,
                main_branch,
                f"{remote}/{main_branch}",
                state["remote_refs"]["main"],
                stage,
            )
            advance(store, state, "end.sync-test")
        elif stage == "end.sync-test":
            current = git.current_branch()
            if current not in (main_branch, test_branch):
                raise WorkflowError(f"同步 test 阶段当前分支异常：{current or 'detached HEAD'}")
            ensure_clean(git, "切换 test 前")
            switch_or_create(git, test_branch, f"{remote}/{test_branch}", stage)
            ensure_clean(git, "同步 test 前")
            sync_shared_branch(
                git,
                test_branch,
                f"{remote}/{test_branch}",
                state["remote_refs"]["test"],
                stage,
            )
            advance(store, state, "end.merge-feature-into-main")
        elif stage == "end.merge-feature-into-main":
            switch_or_create(git, main_branch, f"{remote}/{main_branch}", stage)
            run_merge_stage(
                git,
                store,
                state,
                original_branch,
                stage,
                "end.push-main",
                main_branch,
            )
        elif stage == "end.push-main":
            expect_branch(git, main_branch)
            ensure_clean(git, "推送 main 前")
            remote_before_oid = safe_ref_oid(git, f"{remote}/{main_branch}")
            git.run(["push", remote, main_branch], stage)
            record_push(
                state,
                config,
                stage,
                main_branch,
                git.head(),
                remote_before_oid,
            )
            advance(store, state, "end.merge-main-into-test")
        elif stage == "end.merge-main-into-test":
            switch_or_create(git, test_branch, f"{remote}/{test_branch}", stage)
            run_merge_stage(
                git,
                store,
                state,
                main_branch,
                stage,
                "end.push-test",
                test_branch,
            )
        elif stage == "end.push-test":
            expect_branch(git, test_branch)
            ensure_clean(git, "推送 test 前")
            remote_before_oid = safe_ref_oid(git, f"{remote}/{test_branch}")
            git.run(["push", remote, test_branch], stage)
            record_push(
                state,
                config,
                stage,
                test_branch,
                git.head(),
                remote_before_oid,
            )
            advance(store, state, "end.return-feature")
        elif stage == "end.return-feature":
            current = git.current_branch()
            if current == test_branch:
                ensure_clean(git, "返回 feature 前")
                git.run(["switch", original_branch], stage)
            elif current != original_branch:
                raise WorkflowError(f"返回 feature 阶段当前分支异常：{current or 'detached HEAD'}")
            ensure_clean(git, "coding end 完成检查")
            summary = prepare_summary(
                git, store, state, "end", "success", stage, EXIT_OK
            )
            store.clear()
            print(f"[git-team-workflow] coding end 完成，当前分支：{original_branch}")
            emit_summary(summary)
            return
        else:
            raise WorkflowError(f"未知 end 阶段：{stage}")


def print_conflict_route(
    git: Git,
    store: StateStore,
    exc: ConflictDetected,
    mode: str,
) -> None:
    state = store.load()
    branch = git.current_branch() or "detached HEAD"
    paths = git.conflict_paths()
    print("[git-team-workflow] 检测到 Git 冲突，流程已暂停。", file=sys.stderr)
    print(f"[git-team-workflow] 阶段：{exc.stage}", file=sys.stderr)
    print(f"[git-team-workflow] 当前分支：{branch}", file=sys.stderr)
    if paths:
        print(f"[git-team-workflow] 未合并路径：{', '.join(paths)}", file=sys.stderr)
    if state is not None:
        print(
            f"[git-team-workflow] 请使用 $resolving-merge-conflicts 完成检查、合并、验证并提交；"
            f"完成后重新执行原命令，脚本会从 {exc.stage} 继续。状态文件：{store.path}",
            file=sys.stderr,
        )
    else:
        print(
            "[git-team-workflow] 请使用 $resolving-merge-conflicts 完成冲突；"
            "确认合并提交完成后再重新执行 workflow。",
            file=sys.stderr,
        )
    print(f"[git-team-workflow] 冲突专用退出码：{EXIT_CONFLICT}", file=sys.stderr)
    conflict = {
        "stage": exc.stage,
        "branch": branch,
        "paths": paths,
        "resume_with": "$resolving-merge-conflicts",
        "state_file": str(store.path),
    }
    summary = prepare_summary(
        git,
        store,
        state,
        mode,
        "conflict",
        exc.stage,
        EXIT_CONFLICT,
        conflict=conflict,
    )
    persist_summary(store, state)
    emit_summary(summary, sys.stderr)


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    mode = "begin" if args.command in ("begin", "coding-begin", "codeing-begin") else "end"
    git: Git | None = None
    store: StateStore | None = None
    state: dict[str, Any] | None = None
    try:
        repo = resolve_repo(getattr(args, "repo", None))
        git = Git(repo)
        store = StateStore(git.state_path())
        state = store.load()
        if git.has_conflict():
            stage = state.get("stage", "unknown") if state else "unknown"
            raise ConflictDetected(stage, "仓库中已有未完成的 Git 操作")
        if state is not None and state.get("mode") != mode:
            raise WorkflowError(f"已有未完成的 {state.get('mode')} 流程，请先完成它")
        if mode == "begin":
            run_begin(git, store, args)
        else:
            run_end(git, store, args)
        return EXIT_OK
    except ConflictDetected as exc:
        try:
            if git is None or store is None:
                raise WorkflowError(str(exc))
            print_conflict_route(git, store, exc, mode)
        except (WorkflowError, OSError) as route_error:
            print(f"[git-team-workflow] 无法生成完整冲突提示：{route_error}", file=sys.stderr)
        return EXIT_CONFLICT
    except WorkflowError as exc:
        print(f"[git-team-workflow] 已停止：{exc}", file=sys.stderr)
        summary_state = state
        if summary_state is None and store is not None:
            try:
                summary_state = store.load()
            except WorkflowError:
                summary_state = None
        failure_stage = (
            summary_state.get("stage")
            if summary_state is not None
            else f"{mode}.preflight"
        )
        summary = prepare_summary(
            git,
            store,
            summary_state,
            mode,
            "blocked",
            failure_stage,
            EXIT_ERROR,
            blocker={"type": "workflow_error", "message": str(exc)},
            context=getattr(exc, "context", None),
        )
        persist_summary(store, summary_state)
        emit_summary(summary, sys.stderr)
        return EXIT_ERROR
    except KeyboardInterrupt:
        print("[git-team-workflow] 已被用户中断；未自动回滚或清理状态。", file=sys.stderr)
        summary_state = state
        if summary_state is None and store is not None:
            try:
                summary_state = store.load()
            except WorkflowError:
                summary_state = None
        summary = prepare_summary(
            git,
            store,
            summary_state,
            mode,
            "interrupted",
            summary_state.get("stage") if summary_state is not None else None,
            130,
            blocker={"type": "keyboard_interrupt", "message": "用户中断"},
        )
        persist_summary(store, summary_state)
        emit_summary(summary, sys.stderr)
        return 130


if __name__ == "__main__":
    raise SystemExit(main())
