# Make 构建包含全部界面的通用程序；启动时由 GEER_AGENT_UI 选择。
.DEFAULT_GOAL := help

CARGO ?= cargo
BUN ?= bun
PYTHON ?= python3
FRONTEND := src/ui/frontend
SAFE_RUN := $(abspath scripts/test-safe.sh)
# 间接引用避免 make -n 执行受限服务的准备命令。
MAKE_IN_SERVICE := $(MAKE)

# FEATURES 只用于 test/clippy/doc，默认检查轻量终端配置。
FEATURES ?=
CARGO_FEATURES := $(if $(FEATURES),--features $(FEATURES),)
ARGS ?=
RUN_ARGS := $(if $(ARGS),-- $(ARGS),)
TEST ?=
TEST_ARGS ?=

.PHONY: help build release run frontend-deps frontend-build frontend-check frontend-test \
	test web-test test-safety fmt fmt-check clippy clippy-all check doc clean

help:
	@echo "geer-agent：统一构建，启动时用 GEER_AGENT_UI 选择界面"
	@echo
	@echo "构建与运行（自动准备前端，包含 GUI/Web/TUI/REPL）："
	@echo "  make build          cargo build --features gui,web"
	@echo "                      debug 产物：target/debug/geer-agent[.exe]，不启动"
	@echo "  make release        cargo build --release --features gui,web"
	@echo "                      release 产物：target/release/geer-agent[.exe]，不启动"
	@echo "  make run            cargo run --features gui,web（debug 构建并启动）"
	@echo "  Cargo 的 release 构建是 cargo build --release，没有 cargo release 命令"
	@echo
	@echo "启动界面（进程 ENV 优先于 .env）："
	@echo "  GEER_AGENT_UI=gui|web|tui|repl|auto"
	@echo "  GEER_AGENT_UI=web make run       Web 默认 0.0.0.0:8827"
	@echo "  auto：交互终端进入 TUI，管道输入输出进入 REPL"
	@echo "  ARGS=hello          传给 make run 启动的程序"
	@echo "  仅需终端：直接 cargo build / cargo run，无需前端"
	@echo
	@echo "前端（共用一个包；仅生成静态资源）："
	@echo "  make frontend-deps  Bun 冻结安装"
	@echo "  make frontend-build 构建 GUI 与 Web 静态前端"
	@echo "  make frontend-check 前端类型检查"
	@echo "  make frontend-test  受限前端测试"
	@echo
	@echo "质量检查（默认终端配置；FEATURES=web 或 gui,web 可覆盖）："
	@echo "  make test           受限 cargo test（Linux/systemd）"
	@echo "  make web-test       前端 + 受限 Rust Web 测试"
	@echo "  make test-safety    临时目录隔离与异常清理回归（Python 3）"
	@echo "  make fmt            cargo fmt --all"
	@echo "  make fmt-check      仅检查格式"
	@echo "  make clippy         受限 cargo clippy --all-targets"
	@echo "  make clippy-all     准备前端并检查 gui,web"
	@echo "  make check          fmt -> test-safety -> test -> clippy，顺序执行"
	@echo "  TEST=name / TEST_ARGS='--test tool_loop'  过滤测试"
	@echo "  GEER_TEST_MEMORY_MAX=4G / GEER_TEST_TASKS_MAX=256 / GEER_TEST_RUNTIME_MAX=10min"
	@echo "  make doc / clean    Cargo 文档 / 删除构建产物"

build: frontend-build
	$(CARGO) build --features gui,web

release: frontend-build
	$(CARGO) build --release --features gui,web

run: frontend-build
	$(CARGO) run --features gui,web $(RUN_ARGS)

frontend-deps:
	cd $(FRONTEND) && $(BUN) install --frozen-lockfile --concurrent-scripts 1

frontend-build: frontend-deps
	cd $(FRONTEND) && $(BUN) run build:gui && $(BUN) run build:web

frontend-check: frontend-deps
	cd $(FRONTEND) && $(BUN) run check

frontend-test:
	"$(SAFE_RUN)" $(MAKE_IN_SERVICE) frontend-deps BUN="$(BUN)"
	cd $(FRONTEND) && "$(SAFE_RUN)" $(BUN) run test

test:
	"$(SAFE_RUN)" $(CARGO) test $(CARGO_FEATURES) $(TEST_ARGS) $(TEST)

web-test: frontend-test
	"$(SAFE_RUN)" $(MAKE_IN_SERVICE) frontend-build BUN="$(BUN)"
	$(MAKE) test FEATURES=web TEST="$(TEST)" TEST_ARGS="$(TEST_ARGS)"

test-safety:
	GEER_TEST_MEMORY_MAX=256M GEER_TEST_TASKS_MAX=64 GEER_TEST_RUNTIME_MAX=90s "$(SAFE_RUN)" $(PYTHON) "$(abspath scripts/test-safe-check.py)"

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

clippy:
	"$(SAFE_RUN)" $(CARGO) clippy --all-targets $(CARGO_FEATURES)

# 不用 --all-features，避免意外将 .env 编入程序。
clippy-all:
	"$(SAFE_RUN)" $(MAKE_IN_SERVICE) frontend-build BUN="$(BUN)"
	"$(SAFE_RUN)" $(CARGO) clippy --all-targets --features gui,web

check:
	$(MAKE) fmt
	$(MAKE) test-safety
	$(MAKE) test
	$(MAKE) clippy

doc:
	$(CARGO) doc --no-deps $(CARGO_FEATURES)

clean:
	$(CARGO) clean
	rm -rf $(FRONTEND)/dist
