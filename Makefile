.PHONY: build test clean deploy-testnet deploy-mainnet

build:
	cargo build --release

test:
	cargo test

clean:
	rm -rf target

deploy-testnet:
	./scripts/deploy.sh testnet

deploy-mainnet:
	./scripts/deploy.sh mainnet
