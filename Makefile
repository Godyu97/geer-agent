# geer-agent 开发入口。需要 GNU Make（Windows 可用 Git for Windows 自带的 make）。
# 默认不启用 gui / embed-env：前者要先构建前端，后者会在编译期 include 项目根目录 .env。

.DEFAULT_GOAL := help

CARGO ?= cargo
NPM ?= npm
PYTHON ?= python3
FRONTEND := src/ui/gui/frontend
SAFE_RUN := $(abspath scripts/test-safe.sh)
# 服务中的 make 不继承 jobserver；间接引用也避免 make -n 执行受限服务的准备命令。
MAKE_IN_SERVICE := $(MAKE)

# 可选 Cargo feature，例如：make run FEATURES=gui
FEATURES ?=
CARGO_FEATURES := $(if $(FEATURES),--features $(FEATURES),)

# 传给二进制：make run ARGS=hello（MSYS 下不要写 /help，会被当成路径）
ARGS ?=
RUN_ARGS := $(if $(ARGS),-- $(ARGS),)

# 按名称过滤：make test TEST=query_dependencies_are_resolved_by_configured_bash_only
TEST ?=
# 选择集成测试目标：make test TEST_ARGS='--test tool_loop'
TEST_ARGS ?=

.PHONY: help build run test test-safety fmt fmt-check clippy clippy-all check \
	release embed gui-deps gui-frontend gui-check gui-test gui doc clean

help:
	@echo "geer-agent 开发入口"
	@echo
	@echo "  make build          cargo build"
	@echo "  make run            cargo run"
	@echo "  make test           在独立 cgroup 中运行 cargo test（需要 Linux/systemd）"
	@echo "  make test-safety    验证临时目录隔离与异常清理（需要 Python 3）"
	@echo "  make fmt            cargo fmt --all"
	@echo "  make fmt-check      仅检查格式，不改文件"
	@echo "  make clippy         cargo clippy --all-targets"
	@echo "  make clippy-all     先构建 GUI 前端，再 clippy --features gui"
	@echo "  make check          收工质量门：fmt -> test-safety -> test -> clippy"
	@echo "  make release        cargo build --release"
	@echo "  make embed          cargo build --release --features embed-env"
	@echo "  make gui-deps       前端 npm ci"
	@echo "  make gui-frontend   构建 GUI 静态前端"
	@echo "  make gui-check      前端 tsc --noEmit"
	@echo "  make gui-test       前端 vitest"
	@echo "  make gui            构建前端并以 --features gui 运行"
	@echo "  make doc            cargo doc --no-deps"
	@echo "  make clean          cargo clean，并删除前端 dist"
	@echo
	@echo "变量："
	@echo "  FEATURES=gui        启用 Cargo feature（可逗号分隔）"
	@echo "  ARGS=hello          传给二进制（run / gui）"
	@echo "  TEST=name           按名称过滤测试"
	@echo "  TEST_ARGS='--test tool_loop'  选择 Cargo 测试目标"
	@echo "  GEER_TEST_MEMORY_MAX=4G / GEER_TEST_TASKS_MAX=256 / GEER_TEST_RUNTIME_MAX=10min"

build:
	$(CARGO) build $(CARGO_FEATURES)

run:
	$(CARGO) run $(CARGO_FEATURES) $(RUN_ARGS)

test:
	"$(SAFE_RUN)" $(CARGO) test $(CARGO_FEATURES) $(TEST_ARGS) $(TEST)

test-safety:
	GEER_TEST_MEMORY_MAX=256M GEER_TEST_TASKS_MAX=64 GEER_TEST_RUNTIME_MAX=90s "$(SAFE_RUN)" $(PYTHON) "$(abspath scripts/test-safe-check.py)"

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

clippy:
	"$(SAFE_RUN)" $(CARGO) clippy --all-targets $(CARGO_FEATURES)

# --all-features 会同时打开 embed-env，缺 .env 时无法编译；这里只加 gui。
clippy-all:
	"$(SAFE_RUN)" $(MAKE_IN_SERVICE) gui-frontend NPM="$(NPM)"
	"$(SAFE_RUN)" $(CARGO) clippy --all-targets --features gui

# 安全入口回归先于项目测试；不加 -D warnings。
check:
	$(MAKE) fmt
	$(MAKE) test-safety
	$(MAKE) test
	$(MAKE) clippy

release:
	$(CARGO) build --release $(CARGO_FEATURES)

# 需要项目根目录已有 .env；产物可从二进制提取明文配置。
embed:
	$(CARGO) build --release --features embed-env

gui-deps: $(FRONTEND)/node_modules

$(FRONTEND)/node_modules: $(FRONTEND)/package-lock.json $(FRONTEND)/package.json
	cd $(FRONTEND) && $(NPM) ci

gui-frontend: $(FRONTEND)/node_modules
	cd $(FRONTEND) && $(NPM) run build

gui-check: $(FRONTEND)/node_modules
	cd $(FRONTEND) && $(NPM) run check

gui-test:
	"$(SAFE_RUN)" $(MAKE_IN_SERVICE) gui-deps NPM="$(NPM)"
	cd $(FRONTEND) && "$(SAFE_RUN)" $(NPM) test

gui: export GEER_AGENT_UI := gui
gui: gui-frontend
	$(CARGO) run --features gui $(RUN_ARGS)

doc:
	$(CARGO) doc --no-deps $(CARGO_FEATURES)

clean:
	$(CARGO) clean
	rm -rf $(FRONTEND)/dist
