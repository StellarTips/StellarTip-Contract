.PHONY: build test clean fmt lint check check-changelog scout wasm-build doc deploy-testnet deploy-mainnet interact-testnet

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

## Issue #112: verify src/lib.rs's public-API surface diff between BASE_REF and
## HEAD_REF is mirrored by an update to CHANGELOG.md.
## Usage: make check-changelog BASE_REF=origin/main HEAD_REF=HEAD
check-changelog:
	@if [ -z "$(BASE_REF)" ] || [ -z "$(HEAD_REF)" ]; then \
	  echo "Usage: make check-changelog BASE_REF=<ref> HEAD_REF=<ref>" 1>&2; \
	  echo "Example: make check-changelog BASE_REF=origin/main HEAD_REF=HEAD" 1>&2; \
	  exit 2; \
	fi
	BASE_REF=$(BASE_REF) HEAD_REF=$(HEAD_REF) bash scripts/check_changelog.sh

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
