.DEFAULT_GOAL := help

CARGO ?= cargo

.PHONY: help setup build build-release run check test fmt fmt-check clippy doc precheck clean

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
	$(CARGO) check --all-targets --all-features

test: ## Run the test suite
	$(CARGO) test

fmt: ## Format Rust code with nightly rustfmt
	$(CARGO) +nightly fmt

fmt-check: ## Check Rust formatting without modifying files
	$(CARGO) +nightly fmt --check

clippy: ## Lint all targets and features
	$(CARGO) clippy --all-targets --all-features

doc: ## Generate documentation with warnings treated as errors
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps

precheck: ## Run the full local pre-commit gate (including formatting)
	./scripts/precheck.sh

clean: ## Remove Cargo build artifacts
	$(CARGO) clean
