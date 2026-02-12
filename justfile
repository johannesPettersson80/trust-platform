set shell := ["bash", "-lc"]

fmt:
	cargo fmt

clippy:
	cargo clippy --all-targets --all-features

test:
	cargo test -p trust-runtime --test complete_program
	cargo test --all

check:
	cargo check --all

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
