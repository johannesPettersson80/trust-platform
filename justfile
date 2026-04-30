set shell := ["bash", "-lc"]

fmt:
	cargo fmt

clippy:
	cargo clippy --all-targets --all-features

test:
	@if command -v cargo-nextest >/dev/null 2>&1; then \
		./scripts/cargo_test_fast_link.sh nextest run -p trust-runtime --lib; \
	else \
		echo "cargo-nextest missing; falling back to cargo test -p trust-runtime --lib"; \
		./scripts/cargo_test_fast_link.sh test -p trust-runtime --lib; \
	fi

test-integration:
	./scripts/cargo_test_fast_link.sh test -p trust-runtime --tests

test-e2e:
	./scripts/cargo_test_fast_link.sh test -p trust-runtime --test complete_program

test-all:
	./scripts/cargo_test_fast_link.sh test --all

test-fast:
	./scripts/cargo_test_fast_link.sh test -p trust-runtime --lib

test-runtime:
	./scripts/cargo_test_fast_link.sh test -p trust-runtime

test-ui:
	./scripts/cargo_test_fast_link.sh test -p trust-runtime --test web_io_config_integration

test-nextest:
	@if ! command -v cargo-nextest >/dev/null 2>&1; then \
		echo "cargo-nextest is not installed. Install with: cargo install cargo-nextest"; \
		exit 1; \
	fi
	./scripts/cargo_test_fast_link.sh nextest run -p trust-runtime --lib

check:
	cargo check --all

test-hir-fast:
	./scripts/cargo_test_fast_link.sh test -p trust-hir --lib
	./scripts/cargo_test_fast_link.sh test -p trust-hir --test semantic_type_checking
	./scripts/cargo_test_fast_link.sh test -p trust-hir --test namespaces

editor-smoke:
	./scripts/check_editor_integration_smoke.sh

lint: fmt clippy

readme-media:
	./scripts/prepare-readme-media.sh --dir editors/vscode/assets

plant-demo-media:
	./scripts/capture-plant-demo-media.sh

plant-demo-media-pro:
	./scripts/capture-plant-demo-media-pro.sh

filling-line-media-pro:
	./scripts/capture-filling-line-media-pro.sh

filling-line-debug-scene:
	./scripts/capture-filling-line-debug-scene.sh
