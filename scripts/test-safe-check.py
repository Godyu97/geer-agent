#!/usr/bin/env python3
"""用有限的受限服务验证临时文件隔离与异常清理；入口为 make test-safety。"""

import json
import os
from pathlib import Path
import selectors
import subprocess
import sys
import tempfile
import time


RUNNER = Path(__file__).resolve().with_name("test-safe.sh")
PROBE = r"""
import json, os, signal, sys, tempfile, time
from pathlib import Path

mode, host_marker = sys.argv[1:]
fixture = Path(tempfile.mkdtemp(prefix="geer-safe-probe-"))
(fixture / "fixture").write_text("deliberately left for systemd\n")
var_fixture = Path(tempfile.mkdtemp(prefix="geer-safe-probe-", dir="/var/tmp"))
(var_fixture / "fixture").write_text("deliberately left for systemd\n")
cgroup = Path("/sys/fs/cgroup" + Path("/proc/self/cgroup").read_text().strip()[3:])
print(json.dumps({
    "pid": os.getpid(), "cgroup": str(cgroup),
    "tmp_devices": [os.stat(p).st_dev for p in ("/tmp", "/var/tmp")],
    "files": [str(fixture / "fixture"), str(var_fixture / "fixture")],
    "host_visible": Path(host_marker).exists(),
    "temp_env": [os.environ.get(k) for k in ("TMPDIR", "TMP", "TEMP")],
    "temp_dir": tempfile.gettempdir(),
    "limits": [(cgroup / name).read_text().strip()
               for name in ("memory.max", "memory.swap.max", "pids.max")],
}), flush=True)
# 让控制端先收到就绪信息；不使用无界输出或递归派生。
time.sleep(0.2)
if mode == "failure":
    raise RuntimeError("bounded failure probe")
if mode == "sigkill":
    os.kill(os.getpid(), signal.SIGKILL)
if mode == "oom":
    # 固定最多 96 MiB；已确认的 64 MiB cgroup 上限会先触发组内 OOM。
    data = [b"x" * (1024 * 1024) for _ in range(96)]
if mode in ("timeout", "launcher-term", "launcher-kill"):
    time.sleep(30)
"""


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def check_probe(mode, host_marker):
    env = os.environ.copy()
    env.update(
        GEER_TEST_MEMORY_MAX="64M",
        GEER_TEST_TASKS_MAX="32",
        GEER_TEST_RUNTIME_MAX="2s",
        # 启动入口必须覆盖调用者提供的临时目录，不能向外部目录泄漏。
        TMPDIR=str(host_marker.parent), TMP=str(host_marker.parent),
        TEMP=str(host_marker.parent),
    )
    launcher = subprocess.Popen(
        [str(RUNNER), sys.executable, "-c", PROBE, mode, str(host_marker)],
        env=env, stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
    )
    unit = f"geer-agent-test-{launcher.pid}.service"
    try:
        with selectors.DefaultSelector() as ready:
            ready.register(launcher.stdout, selectors.EVENT_READ)
            require(ready.select(timeout=6), f"{mode}: 服务未按时就绪")
            line = launcher.stdout.readline()
        require(line, f"{mode}: 服务启动失败")
        report = json.loads(line)
        require(report["limits"] == ["67108864", "0", "32"], f"{mode}: 额度未生效")
        require(report["temp_env"] == ["/tmp"] * 3, f"{mode}: 临时目录环境未覆盖")
        require(report["temp_dir"] == "/tmp", f"{mode}: 标准库临时目录未隔离")
        require(not report["host_visible"], f"{mode}: 能访问上一层的临时文件")
        for path, device in zip(("/tmp", "/var/tmp"), report["tmp_devices"]):
            require(os.stat(path).st_dev != device, f"{mode}: {path} 未使用独立 tmpfs")
        for path in report["files"]:
            require(not Path(path).exists(), f"{mode}: 临时文件泄漏到上一层")

        if mode == "launcher-term":
            launcher.terminate()
        elif mode == "launcher-kill":
            launcher.kill()
        _, stderr = launcher.communicate(timeout=12)
        require((launcher.returncode == 0) == (mode == "success"),
                f"{mode}: 退出结果不符: {launcher.returncode}\n{stderr}")
        if mode == "oom":
            require("oom-kill" in stderr, f"{mode}: 未触发受限组内 OOM\n{stderr}")
        if mode in ("timeout", "launcher-kill"):
            require("timeout" in stderr, f"{mode}: 服务运行时限未生效\n{stderr}")

        # namespace 随最后一个进程退出而销毁；不打开 namespace/file fd 延长其寿命。
        process = Path(f"/proc/{report['pid']}")
        cgroup = Path(report["cgroup"])
        deadline = time.monotonic() + 3
        while (process.exists() or cgroup.exists()) and time.monotonic() < deadline:
            time.sleep(0.05)
        require(not process.exists() and not cgroup.exists(), f"{mode}: 服务资源未回收")
        require(host_marker.read_text() == "keep\n", f"{mode}: 上一层临时文件被改动")
        print(f"PASS {mode}: 独立 tmpfs 与测试组均已回收", flush=True)
    finally:
        # 断言失败也只清理这一轮命名服务，不全局删除 /tmp 或终止同用户进程。
        subprocess.run(
            ["systemctl", "--user", "stop", unit], stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=8,
            check=False,
        )
        if launcher.poll() is None:
            launcher.terminate()
        launcher.communicate(timeout=8)


def main():
    host_id = os.environ.get("GEER_TEST_HOST_TMP_ID", "").split(":")[0]
    require(host_id and host_id != str(os.stat("/tmp").st_dev),
            "请用 make test-safety 在受限且隔离的服务中运行")
    env = os.environ.copy()
    tmp_stat = os.stat("/tmp")
    env["GEER_TEST_HOST_TMP_ID"] = f"{tmp_stat.st_dev}:{tmp_stat.st_ino}"
    rejected = subprocess.run(
        [str(RUNNER), "--inside-cgroup", sys.executable, "-c", "print('must not run')"],
        env=env, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=8,
        check=False,
    )
    require(rejected.returncode != 0 and not rejected.stdout and
            "未使用独立 tmpfs" in rejected.stderr, "隔离预检失败时仍执行了测试")
    print("PASS preflight: 隔离未生效时拒绝执行测试", flush=True)
    with tempfile.TemporaryDirectory(prefix="geer-safe-check-") as root:
        marker = Path(root) / "host-marker"
        marker.write_text("keep\n")
        for mode in ("success", "failure", "sigkill", "timeout", "oom",
                     "launcher-term", "launcher-kill"):
            check_probe(mode, marker)


if __name__ == "__main__":
    main()
