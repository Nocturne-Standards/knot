# Warden contract WASM. Own workspace (see Cargo.toml).

WASM_TARGET := wasm32-unknown-unknown
CRATE_NAME := knot_warden
CONTRACT_TARGET_DIR := target/contract
DD_TARGET_DIR := target/data-driver
CONTRACT_WASM := $(CONTRACT_TARGET_DIR)/$(WASM_TARGET)/release/$(CRATE_NAME).wasm
DD_WASM := $(DD_TARGET_DIR)/$(WASM_TARGET)/release/$(CRATE_NAME).wasm
STACK_SIZE ?= 65536
WASM_OPT ?= $(shell command -v wasm-opt 2>/dev/null)

.PHONY: wasm wasm-dd echo echo-dd test clippy

wasm:
	CARGO_TARGET_DIR=$(CONTRACT_TARGET_DIR) \
	RUSTFLAGS="$(RUSTFLAGS) --remap-path-prefix $(HOME)= -C link-args=-zstack-size=$(STACK_SIZE)" \
	cargo build --release --features contract --target $(WASM_TARGET)
	@if [ -n "$(WASM_OPT)" ]; then wasm-opt -Oz --strip-debug $(CONTRACT_WASM) -o $(CONTRACT_WASM); fi
	@echo "Contract WASM: $(CONTRACT_WASM)"

wasm-dd:
	CARGO_TARGET_DIR=$(DD_TARGET_DIR) \
	RUSTFLAGS="$(RUSTFLAGS) --remap-path-prefix $(HOME)=" \
	cargo build --release --features data-driver-js --target $(WASM_TARGET)
	@if [ -n "$(WASM_OPT)" ]; then wasm-opt -Oz --strip-debug $(DD_WASM) -o $(DD_WASM); fi
	@echo "Data-driver WASM: $(DD_WASM)"

echo:
	@echo $(CURDIR)/$(CONTRACT_WASM)

echo-dd:
	@echo $(CURDIR)/$(DD_WASM)

test: wasm
	$(MAKE) -C test-proxy wasm
	$(MAKE) -C ../../../atlas/crates/atlas wasm
	cargo test --release --manifest-path ../knot-warden-encoding/Cargo.toml
	cargo test --release

clippy:
	cargo clippy --release --features contract --target $(WASM_TARGET) -- -D warnings
	cargo clippy --release --manifest-path ../knot-warden-encoding/Cargo.toml --all-targets -- -D warnings
	cargo clippy --release --tests -- -D warnings
