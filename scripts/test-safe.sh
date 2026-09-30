#!/bin/sh
set -eu

# /bin/sh 的非交互入口不读取 Bash 启动钩子；子命令也不继承远程启动条件。
unset BASH_ENV ENV SSH_CLIENT SSH_CONNECTION SSH_TTY

if [ "${1-}" = "--inside-cgroup" ]; then
    shift
    IFS= read -r test_cgroup < /proc/self/cgroup
    case "$test_cgroup" in
        0::*) test_cgroup=/sys/fs/cgroup${test_cgroup#0::} ;;
        *) echo "测试需要 cgroup v2，拒绝无资源限制运行。" >&2; exit 1 ;;
    esac
    # 用户服务管理器可能没有获得控制器委派；检查内核中的实际限制，不能只相信参数。
    for test_limit in memory.max pids.max; do
        IFS= read -r test_value < "$test_cgroup/$test_limit"
        case "$test_value" in
            ''|max|0|*[!0-9]*) echo "$test_limit 未设置有效限制，拒绝运行测试。" >&2; exit 1 ;;
        esac
    done
    IFS= read -r test_swap < "$test_cgroup/memory.swap.max"
    if [ "$test_swap" != 0 ]; then
        echo "测试 cgroup 必须禁用 swap，拒绝运行测试。" >&2
        exit 1
    fi
    # 不依赖测试的析构或 trap：独立 tmpfs 在服务结束时由 systemd 回收。
    # 命名空间功能可能不可用，必须核对实际挂载，不能只相信 PrivateTmp 参数。
    for test_tmp in /tmp /var/tmp; do
        case "$test_tmp" in
            /tmp) test_host_tmp=${GEER_TEST_HOST_TMP_ID-} ;;
            /var/tmp) test_host_tmp=${GEER_TEST_HOST_VAR_TMP_ID-} ;;
        esac
        test_tmp_id=$(/usr/bin/stat -Lc '%d:%i' "$test_tmp")
        test_tmp_type=$(/usr/bin/stat -fLc '%T' "$test_tmp")
        if [ -z "$test_host_tmp" ] || [ "$test_tmp_type" != tmpfs ] || \
            [ "${test_tmp_id%%:*}" = "${test_host_tmp%%:*}" ]; then
            echo "$test_tmp 未使用独立 tmpfs，拒绝运行测试。" >&2
            exit 1
        fi
    done
    if [ "${TMPDIR-}" != /tmp ] || [ "${TMP-}" != /tmp ] || [ "${TEMP-}" != /tmp ]; then
        echo "测试临时目录环境未隔离，拒绝运行测试。" >&2
        exit 1
    fi
    exec "$@" </dev/null
fi

if [ "$#" -eq 0 ]; then
    echo "用法：scripts/test-safe.sh <命令> [参数...]" >&2
    exit 2
fi
if [ "$(uname -s)" != Linux ] || ! command -v systemd-run >/dev/null 2>&1; then
    echo "安全测试入口需要 Linux + systemd 用户服务 + cgroup v2；请在有等效资源限制的隔离环境中测试。" >&2
    exit 1
fi

test_runtime=${GEER_TEST_RUNTIME_MAX:-10min}
case "$test_runtime" in
    *min) test_duration=${test_runtime%min} ;;
    *s) test_duration=${test_runtime%s} ;;
    *h) test_duration=${test_runtime%h} ;;
    *) test_duration=$test_runtime ;;
esac
case "$test_duration" in
    ''|*[!0-9]*) echo "测试运行时限须为正整数秒、s、min 或 h，不能取消时限。" >&2; exit 2 ;;
esac
if ! [ "$test_duration" -gt 0 ]; then
    echo "测试运行时限必须大于零。" >&2
    exit 2
fi

test_program=$(command -v "$1")
shift
case "$test_program" in
    /*) ;;
    *) test_program=$(pwd -P)/$test_program ;;
esac
case "$0" in
    /*) test_script=$0 ;;
    *) test_script=$(pwd -P)/$0 ;;
esac
test_unit=geer-agent-test-$$.service
test_host_tmp=$(/usr/bin/stat -Lc '%d:%i' /tmp)
test_host_var_tmp=$(/usr/bin/stat -Lc '%d:%i' /var/tmp)

# 只停止本次测试服务；不要降低整个用户 slice 的限制或杀同用户的其它进程。
cleanup() {
    systemctl --user stop "$test_unit" >/dev/null 2>&1 || :
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

systemd-run --user --wait --pipe --collect --service-type=exec \
    --unit="$test_unit" --working-directory="$(pwd -P)" \
    --property="MemoryMax=${GEER_TEST_MEMORY_MAX:-4G}" \
    --property=MemorySwapMax=0 \
    --property="TasksMax=${GEER_TEST_TASKS_MAX:-256}" \
    --property="RuntimeMaxSec=$test_runtime" \
    --property=TimeoutStopSec=5s --property=KillMode=control-group \
    --property=OOMPolicy=kill --property=StandardInput=null \
    --property=PrivateTmp=disconnected \
    --property='UnsetEnvironment=BASH_ENV ENV SSH_CLIENT SSH_CONNECTION SSH_TTY' \
    --setenv="PATH=$PATH" \
    --setenv=TMPDIR=/tmp --setenv=TMP=/tmp --setenv=TEMP=/tmp \
    --setenv="GEER_TEST_HOST_TMP_ID=$test_host_tmp" \
    --setenv="GEER_TEST_HOST_VAR_TMP_ID=$test_host_var_tmp" \
    --setenv="CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}" \
    --setenv="RUST_TEST_THREADS=${RUST_TEST_THREADS:-1}" \
    --setenv=FUNCNEST=32 \
    "$test_script" --inside-cgroup "$test_program" "$@"
