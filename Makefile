.PHONY: all build-claude build-codex build-roo build-agents build-gui build-ios build-android clean test

all: build-agents

build-claude:
	cargo build --package remote-code --release

build-codex:
	cd agents/codex/codex-rs && cargo build --package codex-cli --release

build-roo:
	cd agents/roo-code && cargo build --package roo-cli --release

build-agents: build-claude build-codex build-roo

build-gui:
	cd apps/remote-code-gui && npm run tauri build

build-ios:
	cd apps/remote-code-gui && npm run tauri ios build

build-android:
	cd apps/remote-code-gui && npm run tauri android build

dev-gui:
	cd apps/remote-code-gui && npm run tauri dev

dev-ios:
	cd apps/remote-code-gui && npm run tauri ios dev

dev-android:
	cd apps/remote-code-gui && npm run tauri android dev

test:
	cargo test --workspace

clean:
	cargo clean
	cd agents/codex/codex-rs && cargo clean
	cd agents/roo-code && cargo clean
