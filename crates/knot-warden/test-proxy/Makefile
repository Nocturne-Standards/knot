WASM_TARGET := wasm32-unknown-unknown
CRATE_NAME := knot_warden_test_proxy
CONTRACT_TARGET_DIR := target/contract
CONTRACT_WASM := $(CONTRACT_TARGET_DIR)/$(WASM_TARGET)/release/$(CRATE_NAME).wasm
STACK_SIZE ?= 65536
WASM_OPT ?= $(shell command -v wasm-opt 2>/dev/null)

.PHONY: wasm

wasm:
	CARGO_TARGET_DIR=$(CONTRACT_TARGET_DIR) \
	RUSTFLAGS="$(RUSTFLAGS) --remap-path-prefix $(HOME)= -C link-args=-zstack-size=$(STACK_SIZE)" \
	cargo build --release --features contract --target $(WASM_TARGET)
	@if [ -n "$(WASM_OPT)" ]; then wasm-opt -Oz --strip-debug $(CONTRACT_WASM) -o $(CONTRACT_WASM); fi
