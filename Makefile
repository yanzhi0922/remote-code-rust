.PHONY: all build-claude build-codex build-roo build-agents clean test

all: build-agents

build-claude:
	cargo build --package remote-code --release

build-codex:
	cd agents/codex/codex-rs && cargo build --package codex-cli --release

build-roo:
	cd agents/roo-code && cargo build --package roo-cli --release

build-agents: build-claude build-codex build-roo

test:
	cargo test --workspace

clean:
	cargo clean
	cd agents/codex/codex-rs && cargo clean
	cd agents/roo-code && cargo clean
