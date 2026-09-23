---
name: git-team-workflow
description: Safely run team Git coding begin/end workflows with the bundled fast runner, commit-subject CLI arguments, resumable stage state, and dedicated merge-conflict routing. Use when starting work by synchronizing a feature branch with remote main and pushing it, or ending work by committing feature changes, integrating main, promoting main into test, pushing the branches, and returning to the feature branch. Defaults to remote origin and branches main/test; supports explicit overrides and GitLab/GitHub remotes.
---

# Git Team Workflow

Execute a strict, state-aware team Git workflow. Treat an explicit `codeing begin`, `coding begin`, `codeing end`, or `coding end` request as authorization for the fetch, pull, merge, commit, and push operations defined below; request input only when scope or conflict semantics cannot be determined safely.

## Context boundary

- Keep this workflow independent from project business context. Do not require reading `AGENTS.md`, `CLAUDE.md`, `.ai/`, module documentation, OpenSpec changes, source architecture guides, or generated-file instructions.
- Inspect only Git metadata and diffs needed to resolve the repository, branch topology, commit scope, and active Git operation.
- Do not run tests, builds, lint, OpenSpec, documentation synchronization, or other project validation. They are outside this Git workflow unless the user separately requests them.

## Fast runner

Use `scripts/git_team_workflow.py` for the normal path. It performs the Git subcommands in one local Python invocation and stores resumable state under `.git/git-team-workflow-state.json`.

Invoke it from the repository root, or pass `--repo` explicitly:

```text
python -X utf8 .agents/skills/git-team-workflow/scripts/git_team_workflow.py begin
python -X utf8 .agents/skills/git-team-workflow/scripts/git_team_workflow.py end --commit-message "feat: (task)增加任务能力" --path internal/logic/task --path api/task
```

For multiple feature commits in one workflow, repeat `--commit-group`; the first value is the subject and the remaining values are that commit's reviewed paths:

```text
python -X utf8 .agents/skills/git-team-workflow/scripts/git_team_workflow.py end `
  --commit-group "docs: (api)更新接口说明" docs/api.md docs/schema.md `
  --commit-group "test: (task)补充任务用例" internal/logic/task/task_test.go
```

Launch the runner through a terminal tool with the initial tool wait/yield set to `30000` ms (for example, `yield_time_ms=30000`) to avoid an unnecessary second poll when normal Git execution exceeds the default 10 seconds. This controls only how long the tool waits before returning output; it does not shorten or interrupt Git execution. If the command is still running, poll the returned session instead of starting the runner again.

For a clean worktree, omit commit options; the runner never creates an empty commit. Use repeated `--path` values for one explicitly reviewed scope, or repeated `--commit-group "<subject>" <path> [<path> ...]` values for multiple commits. Every changed path must belong to exactly one group. Use `--all` only after confirming that every tracked and untracked change belongs to the requested feature and the user explicitly authorizes the complete scope. Pass `--remote`, `--main`, or `--test` for non-default names. This skill does not run tests, builds, lint, documentation synchronization, or other project validation, and `coding end` does not require them unless the user separately requests validation for the development task.

Exit codes are `0` for success, `1` for a non-conflict stop, and `3` for a Git conflict. The runner never resets, force-pushes, stashes, aborts, or chooses conflict content automatically.

After every workflow execution that reaches the runner, it emits one machine-readable line:

```text
[git-team-workflow] SUMMARY_JSON { ... }
```

Parse this JSON as the primary workflow result. The JSON payload is emitted with ASCII-only `\uXXXX` escapes so Windows console encoding cannot corrupt structured data; parse the payload instead of relying on the rendered diagnostic text. It contains `status`, `stage`, `branches`, `fetched_remote_refs`, local and remote-tracking refs, feature commits, `scope` (`requested_selectors`, normalized `selectors`, `changed_paths`, and `outside_paths`), completed merges with source/result OIDs and `action`, pushes with before/result OIDs and `action`, conflicts, the current blocker, partial remote state, and final worktree state. Even when a commit preflight stops before resumable state is written, the summary retains the requested scope, changed paths, remote, and branch context and uses `stage: "end.preflight"`. Merge actions are `already_up_to_date`, `fast_forward`, or `merge_commit`; push actions are `created`, `updated`, or `already_up_to_date`. Do not run additional Git commands solely to reconstruct these facts. On `success`, use the summary directly; on `conflict` or `blocked`, use its stage, scope, paths, blocker, and partial remote state to decide the next action.

When exit code `3` is returned, stop the runner and use `$resolving-merge-conflicts` (read `.agents/skills/resolving-merge-conflicts/SKILL.md` when the skill is not already loaded). Keep the merge/rebase in progress, let that skill inspect both sides, resolve, validate, stage, and commit it. Then rerun the exact same runner command; the saved state resumes at the interrupted stage. Do not start a second workflow or manually edit the state file.

For a non-conflict failure, correct the reported condition before rerunning. A rejected push, branch divergence, dirty shared branch, or changed remote snapshot is a stop condition, not a conflict-resolution request.

## Scope and path handling

- Prefer repository-relative `--path` values with `/` separators and repeat `--path` for each reviewed file or directory.
- The runner reads Git path lists with NUL delimiters and Unicode normalization, so tracked, staged, and untracked non-ASCII paths are valid scope selectors on Windows and other platforms.
- Use an ASCII parent directory only as an explicitly reviewed fallback when it is confirmed that every changed path below it belongs to the requested commit. Do not broaden scope automatically after a path error.
- A scope mismatch is a non-conflict stop. Inspect the reported paths and correct the selector before rerunning; never use `--all` merely to bypass path encoding or scope errors.

## Preflight

1. Inspect only Git metadata and diffs needed for the workflow; do not load project instruction or business-context files.
2. Resolve the repository root, remote URL, current branch, upstream, worktree status, and active Git operation using read-only Git commands.
3. Default to `remote=origin`, `main=main`, and `test=test`. Apply user-provided overrides consistently.
4. Verify the remote and required remote branches exist after fetching. Stop instead of inventing missing branches.
5. Require a named feature branch. Stop on detached HEAD or when the original branch is `main` or `test`, unless the user explicitly directs that case.
6. Announce the exact source and destination branches before the first merge or push.
7. Treat `test` as a promotion-only branch: accept changes only from `main`; never merge a feature branch directly into `test`, develop on `test`, or use `test` as a source branch. Under this invariant, `main` to `test` should normally be conflict-free.

Use the `github:github` skill only when it is available, the remote is GitHub, and the request also involves repository, PR, issue, review, or check context. Keep local branch, commit, merge, and push work in this skill. Do not load GitHub tooling for GitLab or other remotes, or for local-only Git operations.

## Safety rules

- Never run `reset --hard`, force-push, rewrite published history, delete branches, or change Git configuration.
- Never silently stash, discard, or commit unrelated user changes.
- Prefer explicit file paths when staging. Use `git add -A` only when the complete worktree is confirmed to belong to the requested change.
- Use `git fetch` plus explicit merges instead of an ambiguous `git pull` on feature branches.
- Update shared local branches with fast-forward-only pulls. Stop if local `main` or `test` has diverged from its remote counterpart.
- Never continue to a later promotion stage after an unresolved conflict, failed workflow prerequisite, or rejected push.
- Never claim functional correctness solely because conflict markers were removed or Git commands succeeded.
- Do not automatically undo a successful remote push if a later stage fails. Report partial completion precisely.

## Task-owned validation

- This skill never runs project tests, builds, lint, documentation synchronization, `git diff --check`, or other task validation. Invoking `end` does not require those checks; run them only when the user separately requests validation for the development task.
- The runner checks only Git state, commit scope, branch invariants, merge state, remote snapshots, and resumable workflow state.
- A conflict-free Git operation is sufficient for this skill to continue to the next workflow stage. Do not add project validation between fetch, merge, branch switch, and push stages.
- If conflict handling requires validation, route to `$resolving-merge-conflicts`; do not duplicate its validation workflow here.

## Coding begin

Use this workflow for `codeing begin`, `coding begin`, `开始开发`, or an equivalent explicit request.

Prefer the fast runner above. If it cannot be used, follow the manual invariant below; do not mix a partially completed manual flow with a fresh runner invocation unless the runner reports the current stage as resumable.

1. Save the current branch as `original_branch`.
2. Require a clean worktree and index. If changes already exist, report them and stop without stashing or committing.
3. Fetch current remote state:

   ```text
   git fetch origin --prune
   ```

4. If `origin/<original_branch>` exists, update the current feature branch from its same-named remote branch using fast-forward only:

   ```text
   git merge --ff-only origin/<original_branch>
   ```

   Stop on divergence; do not create a pull merge commit or rebase automatically. If the remote feature branch does not exist yet, skip this step and establish it during the final push.
5. Merge the fetched mainline into the current feature branch:

   ```text
   git merge --no-edit origin/main
   ```

6. If conflicts occur, stop and use `$resolving-merge-conflicts`; after its merge commit, rerun the runner or continue the manual invariant from the current stage.
7. Do not run task validation in this skill. If a conflict was resolved, continue according to `$resolving-merge-conflicts`; otherwise continue directly to the push.
8. Push the current feature branch and establish upstream when needed:

   ```text
   git push -u origin <original_branch>
   ```

9. Confirm the final branch, merge result, and pushed commit. Do not run task validation from this skill.

## Coding end

Use this workflow for `codeing end`, `coding end`, `结束开发`, `收工`, or an equivalent explicit request.

Prefer the fast runner above. Before invoking it with a dirty worktree, inspect the staged/unstaged/untracked scope and identify the files belonging to each commit. Supply one scope with repeated `--path`, or supply multiple reviewed groups with repeated `--commit-group "<subject>" <path> [<path> ...]`. Use `--all` only when the complete worktree is task-owned and the user explicitly authorizes it. Project validation is outside this Git workflow and is not a prerequisite for `coding end` unless the user separately requests it.

### 1. Commit the current feature branch

1. Save the current feature branch as `original_branch`.
2. Inspect `git status`, unstaged diff, staged diff, and recent commit subjects. Identify the files belonging to this task.
3. Inspect all local changes and partition them into coherent commit groups. Each changed path must belong to exactly one group. If generated files belong to different tasks, do not combine them merely to avoid another commit, and do not stash or discard them automatically.
4. For one group, pass its subject with `--commit-message` and its reviewed paths with repeated `--path`. For multiple groups, pass one `--commit-group "<subject>" <path> [<path> ...]` per group; the runner stages and commits them sequentially, recording progress after each commit. If the worktree is already clean, do not create an empty commit.
5. Format generated commit subjects as:

   ```text
   <type>: (<scope>)<summary>
   ```

   Use one of `feat`, `fix`, `refactor`, `docs`, `test`, `perf`, `build`, `ci`, `chore`, or `revert`. Require a lower-case kebab-case scope; use `repo` for repository-wide changes. Put exactly one space after the colon and no space between `)` and the summary. Keep the complete subject on one line, at most 72 characters, without trailing punctuation. Follow the repository's dominant summary language; use concise Chinese when no convention is evident. Validate the subject against `^(feat|fix|refactor|docs|test|perf|build|ci|chore|revert): \([a-z0-9]+(?:-[a-z0-9]+)*\).+$` before committing. Add a body or footer only when it materially explains behavior, migration, issue linkage, or a breaking change.

   Examples:

   ```text
   feat: (schedule)增加第三方日历同步
   fix: (task-reward)修复奖励重复发放
   docs: (repo)更新本地开发说明
   ```
6. Confirm the worktree and index are clean after all feature commits. Stop on `original_branch` if anything remains. Do not push yet; integrate the fetched mainline first.

### 2. Fetch and integrate main into the feature branch

1. Stay on `original_branch` and fetch current remote state. Treat this fetch as the shared-branch snapshot for the remainder of the workflow:

   ```text
   git fetch origin --prune
   ```

2. Verify `origin/main` exists, then merge it into the current feature branch:

   ```text
   git merge --no-edit origin/main
   ```

3. If conflicts occur, stop and use `$resolving-merge-conflicts` on `original_branch`. Stage the resolved paths and complete the merge commit there; do not switch to `main` to resolve these conflicts. Merge commits are exempt from the feature commit subject format.
4. Do not run task validation in this skill. If a conflict was resolved, continue according to `$resolving-merge-conflicts`.
5. Push the integrated feature branch only after the merge completes successfully:

   ```text
   git push -u origin <original_branch>
   ```

6. Confirm the worktree and index are clean before leaving `original_branch`. If the push is rejected, stop on `original_branch`; do not switch shared branches or rewrite history.

### 3. Refresh local shared branches from the fetched snapshot

1. Do not fetch or pull again here. Reuse the `origin/main` and `origin/test` refs captured in step 2 so any conflict with that mainline was already handled on `original_branch`.
2. Switch to local `main`, or create it to track `origin/main` when it does not yet exist. Update it only by fast-forward from the fetched ref:

   ```text
   git switch main
   git merge --ff-only origin/main
   ```

3. Switch to local `test`, or create it to track `origin/test` when it does not yet exist. Update it only by fast-forward from the fetched ref:

   ```text
   git switch test
   git merge --ff-only origin/test
   ```

4. Stop if either shared branch has local changes, local-only commits, divergence, or a non-fast-forward update requirement. Never reset it to the remote automatically.

### 4. Merge the feature branch tip into main

1. Switch to the refreshed local `main`.
2. Merge the complete tip of `original_branch` into `main`:

   ```text
   git merge --no-edit <original_branch>
   ```

   Treat `original_branch` as the latest feature tip. Merge the branch rather than cherry-picking only its merge commit, so the feature commit and the preceding `origin/main` integration are represented correctly and every feature commit absent from `main` is included.
3. Route conflicts to `$resolving-merge-conflicts`.
4. Do not run task validation in this skill. If a conflict was resolved, continue according to `$resolving-merge-conflicts`.
5. Push after a conflict-free merge or after required conflict validation succeeds:

   ```text
   git push origin main
   ```

### 5. Promote main into test

1. After the main push succeeds, treat local `main` as the exact mainline just published. Do not fetch another mainline before promoting it.
2. Switch to the refreshed local `test` and merge local `main`:

   ```text
   git switch test
   git merge --no-edit main
   ```

3. Expect a clean merge because `test` receives changes only from `main`. If a conflict occurs, treat it as evidence that the promotion-only invariant may have been violated; use `$resolving-merge-conflicts` only when both intended behaviors and the invariant remain clear.
4. Do not run task validation in this skill. If a conflict was resolved, continue according to `$resolving-merge-conflicts`.
5. Push after a conflict-free merge or after required conflict validation succeeds:

   ```text
   git push origin test
   ```

### 6. Return to the feature branch

Switch back to `original_branch` after success. On a conflict, leave the in-progress operation in place for `$resolving-merge-conflicts`; do not abort it. On a non-conflict failure, return to `original_branch` only when switching is safe; otherwise stop and report the exact repository state instead of forcing the switch.

## Conflict routing

Do not duplicate conflict-resolution logic in this skill. `$resolving-merge-conflicts` owns conflict inspection, source-intent analysis, hunk resolution, generated-file handling, validation, staging, and the merge/rebase commit. The runner must remain paused until that skill finishes. Preserve the current branch and in-progress operation; never hide uncertainty behind a syntactically valid resolution.

## Completion report

Use the final `SUMMARY_JSON` line as the primary source and report:

- original branch and final checked-out branch
- fetched remote and source commits
- commit created on the feature branch, if any
- merges completed and resulting commit IDs
- branches pushed successfully
- conflicts resolved, key semantic choices, or the exact blocker
- any partial remote state that remains after a later failure
