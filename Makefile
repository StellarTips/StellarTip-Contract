.PHONY: build test clean fmt lint check scout wasm-build deploy-testnet deploy-mainnet

build:
	cargo build --release

wasm-build:
	cargo build --release --target wasm32-unknown-unknown

test:
	cargo test

fmt:
	cargo fmt --all

lint:
	cargo clippy --target wasm32-unknown-unknown --release -- -D warnings

scout:
	cargo scout-audit

check: fmt lint scout test wasm-build
	@echo "All checks passed!"

clean:
	rm -rf target

deploy-testnet:
	./scripts/deploy.sh testnet

deploy-mainnet:
	./scripts/deploy.sh mainnet
