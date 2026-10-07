#!/usr/bin/env python3
"""Read-only checks for explicitly selected governance Markdown. Python 3.9+."""

import argparse
import json
import os
import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit


INLINE_LINK = re.compile(
    r'!?\[[^\]\n]*\]\(\s*(?:<([^>\n]+)>|((?:\\.|[^\\\s()]|\([^()\n]*\))*))'
    r'''\s*(?:["'][^"']*["']\s*)?\)'''
)
REFERENCE_LINK = re.compile(r"^\s{0,3}\[[^\]]+\]:\s*(?:<([^>]+)>|(\S+))")
PLACEHOLDER = re.compile(r"\{\{[A-Za-z_][A-Za-z0-9_]*\}\}")
CORE_DOCS = ["AGENTS.md", ".ai/00-context.md", ".ai/01-project.md",
             ".ai/02-engineering.md", ".ai/03-workflow.md", ".ai/04-documentation.md",
             "docs/mod/00-index.md"]
MODULE_DOCS = ("00-overview.md", "01-implementation.md", "02-issues.md")


def within(root, path):
    resolved = path.resolve()
    try:
        resolved.relative_to(root)
    except ValueError:
        raise ValueError("path resolves outside the selected root")
    return resolved


def prose_lines(text):
    """Skip fenced examples and inline code; this is not a complete Markdown parser."""
    fence = None
    for number, line in enumerate(text.splitlines(), 1):
        match = re.match(r"^\s*(`{3,}|~{3,})(.*)$", line)
        if match:
            marker, tail = match.groups()
            if fence is None:
                fence = marker
            elif marker[0] == fence[0] and len(marker) >= len(fence) and not tail.strip():
                fence = None
            continue
        if fence is None:
            yield number, re.sub(r"(`+).*?\1", "", line)


def check(root, files, max_lines=None):
    root = root.resolve(strict=True)
    errors, warnings, checked = [], [], []
    external_count, fragments = 0, 0

    def issue(collection, file, line, code, detail):
        collection.append({"file": file, "line": line, "code": code, "detail": detail})

    for name in dict.fromkeys(files):
        try:
            if Path(name).is_absolute():
                raise ValueError("select files using paths relative to the root")
            path = within(root, root / name)
            if not path.is_file():
                raise ValueError("selected file is missing or is not a regular file")
            text = path.read_text(encoding="utf-8")
        except (OSError, ValueError, RuntimeError) as error:
            issue(errors, name, 0, "invalid_file", str(error))
            continue
        lines = len(text.splitlines())
        entry = {"file": name, "bytes": len(text.encode("utf-8")), "lines": lines,
                 "checked_local_links": 0}
        checked.append(entry)
        if not text.strip():
            issue(errors, name, 0, "empty_file", "selected governance file is empty")
        if max_lines is not None and lines > max_lines:
            issue(warnings, name, 0, "size_review", "line count exceeds the requested review threshold")
        for number, line in enumerate(text.splitlines(), 1):
            if PLACEHOLDER.search(line):
                issue(errors, name, number, "template_slot", "unfilled template slot")
        # Resolve links from the displayed document location, not its symlink target.
        origin = root / name
        for number, line in prose_lines(text):
            targets = [m.group(1) if m.group(1) is not None else m.group(2)
                       for m in INLINE_LINK.finditer(line)]
            reference = REFERENCE_LINK.match(line)
            if reference:
                targets.append(reference.group(1) or reference.group(2))
            for target in targets:
                try:
                    parts = urlsplit(target)
                    if parts.scheme or parts.netloc:
                        external_count += 1
                        continue
                    if parts.fragment:
                        fragments += 1
                    if not parts.path:
                        continue
                    decoded = unquote(re.sub(r"\\([() ])", r"\1", parts.path))
                    if decoded.startswith("/"):
                        issue(warnings, name, number, "absolute_or_site_link",
                              "absolute or website-root link needs manual portability review")
                        continue
                    # Common source-file line suffixes, e.g. src/main.py:12.
                    decoded = re.sub(r":\d+(?::\d+)?$", "", decoded)
                    target_path = within(root, origin.parent / decoded)
                    if not target_path.exists():
                        issue(errors, name, number, "missing_target", decoded)
                    else:
                        entry["checked_local_links"] += 1
                except (OSError, ValueError, RuntimeError):
                    issue(errors, name, number, "invalid_target", "invalid path or link escapes the root")
    return {
        "result": "failed" if errors else "mechanical_checks_passed",
        "checked_files": checked, "errors": errors, "warnings": warnings,
        "skipped_external_links": external_count, "unchecked_fragments": fragments,
        "limitations": [
            "Checks only selected files, template slots and common inline/reference links.",
            "Does not verify anchors, HTML/MDX, implicit reference links, recursive imports or client loading.",
            "Does not execute commands or prove policy consistency, code coverage, security or Agent behavior.",
        ],
    }


def local_path(root, value):
    if not isinstance(value, str) or not value or "\\" in value or Path(value).is_absolute():
        raise ValueError("expected a nonempty portable repository-relative path")
    if ".." in Path(value).parts or ".git" in Path(value).parts:
        raise ValueError("parent traversal and .git paths are not allowed")
    return within(root, root / value)


def catalog_errors(root, data, require_docs=True):
    """Validate declared mapping, not the truth of natural-language module descriptions."""
    errors = []
    def problem(detail):
        errors.append({"file": ".ai/catalog.json", "line": 0,
                       "code": "invalid_catalog", "detail": detail})
    if not isinstance(data, dict) or type(data.get("version")) is not int or data["version"] != 1:
        problem("catalog must be an object with version 1")
        return errors
    for field in ("modules", "source_roots", "excluded_paths", "components"):
        if not isinstance(data.get(field), list):
            problem(field + " must be a list")
    if errors:
        return errors
    def existing(value, label, kind=None, optional=False):
        try:
            path = local_path(root, value)
            if not path.exists():
                # 依赖/生成目录在新 checkout 中可不存在，路径仍须合法且留在仓库内。
                if optional:
                    return
                raise ValueError("path does not exist")
            if kind == "file" and not path.is_file():
                raise ValueError("path must be a regular file")
            if kind == "directory" and not path.is_dir():
                raise ValueError("path must be a directory")
        except (ValueError, OSError, RuntimeError) as error:
            problem(label + ": " + str(error))
    for value in data["source_roots"]:
        existing(value, "source_roots")
    for item in data["excluded_paths"]:
        if not isinstance(item, dict) or not isinstance(item.get("reason"), str) or not item["reason"].strip():
            problem("each excluded path needs a reason")
        else:
            existing(item.get("path"), "excluded_paths", optional=True)
    for item in data["components"]:
        if not isinstance(item, dict) or not all(isinstance(item.get(k), str) and item[k].strip()
                                               for k in ("root", "language", "manifest")):
            problem("component needs root, language and manifest")
        else:
            existing(item["root"], "component root", "directory")
            existing(item["manifest"], "component manifest", "file")
    ids = set()
    for module in data["modules"]:
        if not isinstance(module, dict):
            problem("module must be an object")
            continue
        identifier = module.get("id")
        if not isinstance(identifier, str) or not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", identifier):
            problem("module id must be a lowercase slug")
            continue
        if identifier in ids:
            problem("duplicate module id: " + identifier)
        ids.add(identifier)
        for field in ("code_paths", "test_paths"):
            values = module.get(field)
            if not isinstance(values, list) or (field == "code_paths" and not values):
                problem(identifier + ": " + field + " must be a list (code_paths nonempty)")
            else:
                for value in values:
                    existing(value, identifier + ": " + field)
        expected = ["docs/mod/" + identifier + "/" + name for name in MODULE_DOCS]
        if module.get("docs") != expected:
            problem(identifier + ": module document paths do not match its id")
        elif require_docs:
            for value in expected:
                existing(value, identifier + ": docs", "file")
    if data["source_roots"] and not data["modules"]:
        problem("source roots were declared without any owning modules")
    return errors


def beneath(value, prefix):
    value = Path(value).as_posix()
    prefix = Path(prefix).as_posix().rstrip("/")
    return prefix == "." or value == prefix or value.startswith(prefix + "/")


def check_framework(root):
    root = root.resolve(strict=True)
    try:
        catalog_path = local_path(root, ".ai/catalog.json")
        if not catalog_path.is_file():
            raise ValueError("catalog must be a regular file")
        data = json.loads(catalog_path.read_text(encoding="utf-8"))
    except (OSError, ValueError, RuntimeError) as error:
        return {"result": "failed", "errors": [{"file": ".ai/catalog.json", "line": 0,
                "code": "invalid_catalog", "detail": str(error)}], "warnings": []}
    errors = catalog_errors(root, data)
    if errors:
        return {"result": "failed", "errors": errors, "warnings": []}
    documents = CORE_DOCS + [path for module in data["modules"] for path in module["docs"]]
    result = check(root, documents)
    errors = result["errors"]
    warnings = result["warnings"]
    def issue(file, code, detail, warning=False):
        (warnings if warning else errors).append({"file": file, "line": 0,
                                                  "code": code, "detail": detail})
    checker = ".ai/scripts/check_governance.py"
    try:
        if not local_path(root, checker).is_file():
            raise ValueError("repository-local checker is missing")
    except (OSError, ValueError, RuntimeError) as error:
        issue(checker, "missing_checker", str(error))
    # Check actual link destinations so a path mentioned only in prose is insufficient.
    def targets(name):
        found = set()
        try:
            text = local_path(root, name).read_text(encoding="utf-8")
            for _, line in prose_lines(text):
                matches = list(INLINE_LINK.finditer(line))
                definition = REFERENCE_LINK.match(line)
                if definition:
                    matches.append(definition)
                for match in matches:
                    part = urlsplit(match.group(1) or match.group(2))
                    if not part.scheme and not part.netloc and part.path:
                        found.add(within(root, (root / name).parent / unquote(part.path)))
        except (OSError, ValueError, RuntimeError):
            pass  # The ordinary file/link checker reports the underlying error.
        return found
    entry_targets = targets("AGENTS.md")
    context_targets = targets(".ai/00-context.md")
    if root / ".ai/00-context.md" not in entry_targets:
        issue("AGENTS.md", "missing_entry_route", "link to .ai/00-context.md is required")
    if root / "docs/mod/00-index.md" not in entry_targets | context_targets:
        issue(".ai/00-context.md", "missing_index_route", "entry or context must link to the module index")
    for name in CORE_DOCS[2:6]:
        if root / name not in context_targets:
            issue(".ai/00-context.md", "missing_handbook_route", name)
    index_targets = targets("docs/mod/00-index.md")
    for module in data["modules"]:
        if root / module["docs"][0] not in index_targets:
            issue("docs/mod/00-index.md", "missing_module_route", module["id"])
    if not data["modules"]:
        issue("docs/mod/00-index.md", "no_modules", "no implemented module was declared", warning=True)

    source_errors_start = len(errors)
    excluded = data["excluded_paths"]
    sources, unchecked, empty_roots = set(), set(), []
    skipped, root_counts = {}, []

    def skip_reason(value):
        if ".git" in Path(value).parts:
            return "Git metadata"
        if value == "AGENTS.md" or beneath(value, ".ai") or beneath(value, "docs/mod"):
            return "governance artifacts, checked separately"
        for item in excluded:
            if beneath(value, item["path"]):
                return item["reason"]
        return None

    def uninspected(value, detail):
        if value not in unchecked:
            issue(value, "source_symlink_unchecked", detail, warning=True)
            unchecked.add(value)

    def scan_error(error):
        path = Path(error.filename) if error.filename else root
        try:
            relative = path.relative_to(root).as_posix()
        except ValueError:
            relative = "source_roots"
        issue(relative, "source_scan_failed", "cannot inspect a declared source directory")

    for prefix in data["source_roots"]:
        base = root / prefix
        relative = base.relative_to(root).as_posix()
        reason = skip_reason(relative)
        if reason:
            skipped[relative] = reason
            root_counts.append({"root": prefix, "files": 0, "scope": "excluded"})
            continue
        if base.is_symlink():
            uninspected(relative, "declared source root is a symlink; not traversed")
            root_counts.append({"root": prefix, "files": 0, "scope": "unchecked"})
            continue
        if base.is_file():
            sources.add(relative)
            root_counts.append({"root": prefix, "files": 1, "scope": "inspected"})
            continue
        count = 0
        for directory, dirs, files in os.walk(base, followlinks=False, onerror=scan_error):
            retained = []
            for name in dirs:
                candidate = Path(directory) / name
                relative = candidate.relative_to(root).as_posix()
                reason = skip_reason(relative)
                if reason:
                    skipped[relative] = reason
                    continue
                if candidate.is_symlink():
                    uninspected(relative, "directory not traversed")
                    continue
                retained.append(name)
            dirs[:] = retained
            for name in files:
                path = Path(directory) / name
                relative = path.relative_to(root).as_posix()
                reason = skip_reason(relative)
                if reason:
                    skipped[relative] = reason
                elif path.is_symlink():
                    uninspected(relative, "file symlink not inspected")
                elif path.is_file():
                    sources.add(relative)
                    count += 1
                else:
                    issue(relative, "invalid_source_file", "declared source entry is not a regular file")
        root_counts.append({"root": prefix, "files": count, "scope": "inspected"})
        if count == 0:
            empty_roots.append(prefix)
            issue(prefix, "empty_source_root", "no regular file was inspected in this source root", warning=True)
    unmapped, ambiguous, inspected = [], [], 0
    for value in sorted(sources):
        inspected += 1
        try:
            local_path(root, value)
        except (OSError, ValueError, RuntimeError):
            issue(value, "source_escape", "source path resolves outside repository")
            continue
        owners = []
        for module in data["modules"]:
            matches = [p for p in module["code_paths"] if beneath(value, p)]
            if matches:
                owners.append((max(len(Path(p).parts) if p != "." else 0 for p in matches), module["id"]))
        if owners:
            longest = max(item[0] for item in owners)
            if sum(item[0] == longest for item in owners) > 1:
                ambiguous.append(value)
        elif not any(beneath(value, p) for module in data["modules"] for p in module["test_paths"]):
            unmapped.append(value)
    for value in unmapped[:20]:
        issue(value, "unmapped_source", "no module covers this source file")
    for value in ambiguous[:20]:
        issue(value, "ambiguous_owner", "multiple equally specific code owners")
    if data["source_roots"] and not inspected and not (unchecked or empty_roots):
        issue(".ai/catalog.json", "no_inspected_sources", "all declared source roots were excluded", warning=True)
    complete = bool(inspected) and len(errors) == source_errors_start and not (unchecked or empty_roots)
    result["source_coverage"] = {
        "mode": "all_declared_files", "complete": complete,
        "status": "not_declared" if not data["source_roots"] else ("complete" if complete else "incomplete"),
        "checked": inspected, "unmapped": len(unmapped), "ambiguous": len(ambiguous),
        "unchecked_symlinks": len(unchecked), "roots": root_counts,
        "skipped_count": len(skipped),
        "skipped_examples": [{"path": path, "reason": reason} for path, reason in sorted(skipped.items())[:20]],
        "error_examples_limit": 20,
    }
    result["limitations"].append(
        "Ownership checks include every regular file in declared roots, regardless of language or suffix. "
        "Git metadata, governance artifacts and reasoned exclusions are skipped; symlinks are reported as unchecked. "
        "Omitted source roots, semantic module boundaries and the correctness of exclusions still require evidence review."
    )
    result["result"] = "failed" if errors else "mechanical_checks_passed"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, type=Path, help="Root boundary; Git is not required.")
    selection = parser.add_mutually_exclusive_group(required=True)
    selection.add_argument("--files", nargs="+", help="Explicit paths relative to root.")
    selection.add_argument("--framework", action="store_true", help="Validate .ai/catalog.json and the full framework.")
    parser.add_argument("--max-lines", type=int, help="Optional size warning, not a token or pass threshold.")
    args = parser.parse_args()
    if args.max_lines is not None and args.max_lines < 1:
        parser.error("--max-lines must be positive")
    try:
        if args.framework and args.max_lines is not None:
            parser.error("--max-lines is only supported with --files")
        result = check_framework(args.repo) if args.framework else check(args.repo, args.files, args.max_lines)
    except (OSError, ValueError, RuntimeError) as error:
        print(json.dumps({"error": str(error)}), file=sys.stderr)
        return 2
    print(json.dumps(result, ensure_ascii=True, indent=2))
    return 1 if result["errors"] else 0


if __name__ == "__main__":
    sys.exit(main())
