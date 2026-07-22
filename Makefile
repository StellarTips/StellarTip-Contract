.PHONY: build test clean fmt lint check scout wasm-build doc deploy-testnet deploy-mainnet interact-testnet

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

doc:
	cargo doc --no-deps --document-private-items=false

check: fmt lint scout test wasm-build doc
	@echo "All checks passed!"

clean:
	rm -rf target

deploy-testnet:
	./scripts/deploy.sh testnet

deploy-mainnet:
	./scripts/deploy.sh mainnet

## Invoke a function on the deployed testnet contract.
## Usage: make interact-testnet FN=get_contract_version
##        make interact-testnet FN=get_profile ARGS="--address GABC..."
interact-testnet:
	./scripts/interact.sh $(FN) $(ARGS)
