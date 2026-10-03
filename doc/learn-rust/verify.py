#!/usr/bin/env python3
"""检查学习文档；执行例子前必须通过仓库的隔离入口。"""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib
from urllib.parse import unquote


ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent.parent


def blocks(path):
    result = []
    active = None
    body = []
    start = 0
    for number, line in enumerate(path.read_text().splitlines(), 1):
        if active is None and line.startswith("```"):
            active, body, start = line[3:].strip(), [], number
        elif active is not None and line.strip() == "```":
            result.append((active, "\n".join(body), start))
            active = None
        elif active is not None:
            body.append(line)
    if active is not None:
        raise ValueError(f"{path.relative_to(ROOT)}:{start}: unclosed code fence")
    return result


def check_links(path):
    content = re.sub(r"^```[^\n]*\n.*?^```\s*$", "", path.read_text(), flags=re.M | re.S)
    failures = []
    count = 0
    for match in re.finditer(r"\[[^\]\n]+\]\((<[^>]+>|[^\s)]+)\)", content):
        target = match.group(1).strip("<>")
        if re.match(r"[a-zA-Z][a-zA-Z0-9+.-]*:", target) or target.startswith("#"):
            continue
        target = re.sub(r":\d+$", "", unquote(target.split("#", 1)[0]))
        resolved = Path(target) if target.startswith("/") else path.parent / target
        count += 1
        if not resolved.exists():
            failures.append(f"{path.relative_to(ROOT)}: missing link {target}")
    return count, failures


def check_frameworks(run=False):
    examples = {
        "actor": ("08-ecosystem/04-tokio-workshop.md", 0),
        "bounded": ("08-ecosystem/04-tokio-workshop.md", 1),
        "axum": ("08-ecosystem/05-axum-and-tower.md", 0),
        "serde": ("08-ecosystem/06-serde-and-reqwest.md", 0),
        "reqwest": ("08-ecosystem/06-serde-and-reqwest.md", 1),
        "sqlx": ("08-ecosystem/07-database-and-rpc.md", 0),
        "framing": ("08-ecosystem/11-tokio-io-and-shutdown.md", 0),
        "shutdown": ("08-ecosystem/11-tokio-io-and-shutdown.md", 1),
        "web_contracts": ("08-ecosystem/12-web-testing-and-middleware.md", 0),
        "observability": ("08-ecosystem/14-errors-and-observability.md", 0),
    }
    lock = tomllib.loads((REPO / "Cargo.lock").read_text())
    versions = {p["name"]: p["version"] for p in lock["package"]}
    # 练习工程只在本轮私有临时目录；不改主项目依赖与锁文件。
    with tempfile.TemporaryDirectory(prefix="geer-learn-rust-") as directory:
        package = Path(directory)
        binary_dir = package / "src/bin"
        binary_dir.mkdir(parents=True)
        manifest = ['[package]', 'name = "geer-learning-examples"', 'version = "0.0.0"',
                    'edition = "2024"', '', '[dependencies]']
        settings = {
            "axum": None,
            "serde": ['derive'],
            "serde_json": None,
            "tokio": ['macros', 'rt', 'time', 'sync', 'net', 'signal', 'io-util'],
            "tower": ['util', 'timeout'],
            "reqwest": ['rustls', 'json'],
            "sqlx": ['runtime-tokio', 'sqlite'],
            "anyhow": None,
            "thiserror": None,
            "tracing": ['attributes'],
            "tracing-subscriber": ['fmt'],
        }
        for name, features in settings.items():
            spec = f'version = "={versions[name]}"'
            if name in {"reqwest", "sqlx", "tracing-subscriber"}:
                spec += ', default-features = false'
            if features:
                spec += ', features = ' + json.dumps(features)
            manifest.append(f'{name} = {{ {spec} }}')
        (package / "Cargo.toml").write_text("\n".join(manifest) + "\n")
        for name, (source, index) in examples.items():
            snippets = [code for info, code, _ in blocks(ROOT / source)
                        if "rust" in re.split(r"[\s,]+", info)]
            (binary_dir / f"{name}.rs").write_text(snippets[index] + "\n")
        common = ['--offline', '--manifest-path', str(package / "Cargo.toml"),
                  '--target-dir', str(REPO / "target")]
        commands = [(['cargo', 'run', '--quiet', '--bin', name, *common], name)
                    for name in ['actor', 'bounded', 'serde', 'sqlx', 'framing', 'shutdown', 'web_contracts', 'observability']] if run else [
                        (['cargo', 'check', '--bins', *common], 'framework compilation')]
        for command, name in commands:
            result = subprocess.run(command, stdin=subprocess.DEVNULL, capture_output=True,
                                    text=True, timeout=300)
            if result.returncode:
                print(result.stdout[-12000:], file=sys.stderr)
                print(result.stderr[-12000:], file=sys.stderr)
                return result.returncode
            print(f"PASS {name}", flush=True)
    return 0


def check_tauri_commands():
    lock = tomllib.loads((REPO / "Cargo.lock").read_text())
    versions = {p["name"]: p["version"] for p in lock["package"]}
    snippets = [code for info, code, _ in blocks(ROOT / "08-ecosystem/13-tauri-ipc-and-lifecycle.md")
                if "rust" in re.split(r"[\s,]+", info)]
    with tempfile.TemporaryDirectory(prefix="geer-learn-tauri-") as directory:
        package = Path(directory)
        (package / "src").mkdir()
        (package / "src/lib.rs").write_text(snippets[0] + "\n")
        (package / "Cargo.toml").write_text(
            '[package]\nname = "geer-learning-tauri-commands"\nversion = "0.0.0"\nedition = "2024"\n'
            '[dependencies]\n'
            f'tauri = {{ version = "={versions["tauri"]}", default-features = false }}\n'
            f'serde = {{ version = "={versions["serde"]}", features = ["derive"] }}\n'
            f'tokio = {{ version = "={versions["tokio"]}", features = ["time"] }}\n')
        result = subprocess.run(
            ['cargo', 'check', '--offline', '--lib', '--manifest-path', str(package / "Cargo.toml"),
             '--target-dir', str(REPO / "target")],
            stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=300)
        if result.returncode:
            print(result.stdout[-12000:], file=sys.stderr)
            print(result.stderr[-12000:], file=sys.stderr)
            return result.returncode
        print("PASS Tauri command module and handler registration", flush=True)
    return 0


def check_diagrams(diagrams):
    with tempfile.TemporaryDirectory(prefix="geer-learn-mermaid-") as directory:
        package = Path(directory)
        result = subprocess.run(
            ['bun', 'add', '--exact', '--concurrent-scripts', '1', 'mermaid@11', 'jsdom'],
            cwd=package, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=120)
        if result.returncode:
            print(result.stderr[-12000:], file=sys.stderr)
            return result.returncode
        (package / 'diagrams.json').write_text(json.dumps(diagrams, ensure_ascii=False))
        (package / 'parse.mjs').write_text('''
import fs from 'node:fs';
import { JSDOM } from 'jsdom';
const dom = new JSDOM('');
globalThis.window = dom.window;
globalThis.document = dom.window.document;
const { default: mermaid } = await import('mermaid');
mermaid.initialize({ startOnLoad: false, securityLevel: 'strict' });
for (const diagram of JSON.parse(fs.readFileSync('diagrams.json', 'utf8'))) {
  try { await mermaid.parse(diagram.code); }
  catch (error) { console.error(diagram.path + ':' + diagram.line, error); process.exit(1); }
}
const metadata = JSON.parse(fs.readFileSync('node_modules/mermaid/package.json', 'utf8'));
console.log('PASS Mermaid ' + metadata.version);
''')
        result = subprocess.run(['node', 'parse.mjs'], cwd=package, stdin=subprocess.DEVNULL,
                                capture_output=True, text=True, timeout=60)
        if result.returncode:
            print(result.stderr[-12000:], file=sys.stderr)
            return result.returncode
        print(result.stdout.strip(), flush=True)
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--examples", action="store_true", help="在有效隔离内执行 rustdoc 示例")
    parser.add_argument("--frameworks", action="store_true", help="在有效隔离内编译十个框架完整例子")
    parser.add_argument("--run-frameworks", action="store_true", help="在隔离内运行八个有限离线框架程序")
    parser.add_argument("--tauri-commands", action="store_true", help="在隔离内编译 Tauri 命令模块，不启动窗口")
    parser.add_argument("--diagrams", action="store_true", help="在私有临时目录安装 Mermaid 11 并解析图")
    parser.add_argument("--diagrams-output", type=Path, help="输出 Mermaid 块，供临时解析器校验")
    args = parser.parse_args()
    paths = sorted(ROOT.rglob("*.md"))
    errors, diagrams = [], []
    summary = {"markdown_files": len(paths), "local_links": 0,
               "rust_examples": 0, "compile_fail": 0, "should_panic": 0, "ignored": 0}
    example_paths = []
    for path in paths:
        count, failures = check_links(path)
        summary["local_links"] += count
        errors.extend(failures)
        code_blocks = blocks(path)
        has_examples = False
        for info, code, number in code_blocks:
            flags = set(re.split(r"[\s,]+", info))
            if "mermaid" in flags:
                diagrams.append({"path": str(path.relative_to(ROOT)), "line": number, "code": code})
            elif "rust" in flags:
                if "ignore" in flags:
                    summary["ignored"] += 1
                else:
                    has_examples = True
                    category = "compile_fail" if "compile_fail" in flags else (
                        "should_panic" if "should_panic" in flags else "rust_examples")
                    summary[category] += 1
        if has_examples:
            example_paths.append(path)
    summary["mermaid_diagrams"] = len(diagrams)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    if args.diagrams_output:
        args.diagrams_output.write_text(json.dumps(diagrams, ensure_ascii=False, indent=2) + "\n")
    if args.examples or args.frameworks or args.run_frameworks or args.tauri_commands or args.diagrams:
        # 复用安全入口的实际限制检查，拒绝裸执行文档测试。
        subprocess.run([str(REPO / "scripts/test-safe.sh"), "--inside-cgroup", "/usr/bin/true"],
                       check=True, stdin=subprocess.DEVNULL, timeout=10)
    if args.examples:
        for path in example_paths:
            result = subprocess.run(
                ["rustdoc", "--test", "--edition", "2024", str(path),
                 "--test-args=--test-threads=1"],
                stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=60)
            if result.returncode:
                print(result.stdout, file=sys.stderr)
                print(result.stderr, file=sys.stderr)
                return result.returncode
            print(f"PASS {path.relative_to(ROOT)}", flush=True)
        summary["rustdoc_files_passed"] = len(example_paths)
    if args.frameworks:
        status = check_frameworks()
        if status:
            return status
        summary["framework_programs_compiled"] = 10
    if args.run_frameworks:
        status = check_frameworks(run=True)
        if status:
            return status
        summary["framework_programs_executed"] = 8
    if args.tauri_commands:
        status = check_tauri_commands()
        if status:
            return status
        summary["tauri_command_modules_compiled"] = 1
    if args.diagrams:
        status = check_diagrams(diagrams)
        if status:
            return status
        summary["mermaid_diagrams_parsed"] = len(diagrams)
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
