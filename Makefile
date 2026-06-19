.PHONY: build test test-fork clean fmt lint check wasm-build \
        deploy-testnet deploy-mainnet \
        start-testnet stop-testnet capture-snapshot

build:
	cargo build --release

wasm-build:
	cargo build --release --target wasm32-unknown-unknown

test:
	cargo test

test-fork:
	cargo test --test fork

test-all: test test-fork
	@echo "All tests (unit + fork) passed!"

fmt:
	cargo fmt --all

lint:
	cargo clippy --target wasm32-unknown-unknown --release -- -D warnings

check: fmt lint test-all wasm-build
	@echo "All checks passed!"

clean:
	rm -rf target

deploy-testnet:
	./scripts/deploy.sh testnet

deploy-mainnet:
	./scripts/deploy.sh mainnet

start-testnet:
	./scripts/start-testnet.sh

stop-testnet:
	-docker stop stellar-tip-soroban 2>/dev/null; true

capture-snapshot:
	./scripts/capture-snapshot.sh
