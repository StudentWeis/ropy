.DEFAULT_GOAL := help

CARGO ?= cargo

.PHONY: help setup build build-release run check test fmt fmt-check clippy doc precheck clean bench bench-compare

help: ## Show available development commands
	@awk 'BEGIN { FS = ":.*## " } /^[a-zA-Z0-9_-]+:.*## / { printf "  %-18s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

setup: ## Install development tools and Git hooks
	./scripts/init.sh

build: ## Build the debug application
	$(CARGO) build

build-release: ## Build the optimized application
	$(CARGO) build --release

run: ## Build and run the debug application
	$(CARGO) run

check: ## Check all targets and features
	$(CARGO) check --workspace --all-targets --all-features

test: ## Run the test suite
	$(CARGO) test --workspace --all-targets --all-features

fmt: ## Format Rust code with nightly rustfmt
	$(CARGO) +nightly fmt --all

fmt-check: ## Check Rust formatting without modifying files
	$(CARGO) +nightly fmt --all --check

clippy: ## Lint all targets and features
	$(CARGO) clippy --workspace --all-targets --all-features

doc: ## Generate documentation with warnings treated as errors
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --workspace --no-deps

precheck: ## Run the full local pre-commit gate (including formatting)
	./scripts/precheck.sh

clean: ## Remove Cargo build artifacts
	$(CARGO) clean

BENCH_ARGS ?=
BASELINE ?=
CURRENT ?=

bench: ## Collect release size, storage latency and isolated macOS RSS
	python3 scripts/bench.py run $(BENCH_ARGS)

bench-compare: ## Compare with BASELINE=path/to/result.json (optional CURRENT=...)
	python3 scripts/bench.py compare --baseline "$(BASELINE)" $(if $(CURRENT),--current "$(CURRENT)",)
