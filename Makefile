.DEFAULT_GOAL=help

help:
	@cat Makefile

.PHONY: build
build:
	cargo build --target wasm32-unknown-unknown --release

.PHONY: dev-deploy
dev-deploy: build
	yarn near dev-deploy --wasmFile target/wasm32-unknown-unknown/release/prediction_market.wasm
