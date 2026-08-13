MOLD_VERSION_FILE := .mold-version
RUST_TOOLCHAIN_FILE := rust-toolchain.toml

export MOLD_VERSION_FILE
export RUST_TOOLCHAIN_FILE
export CARGO_TERM_COLOR := always

build:
	@echo building
