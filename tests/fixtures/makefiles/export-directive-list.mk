MOLD_VERSION_FILE := .mold-version
MOLD_SHA256SUMS_FILE := .mold-sha256sums
RUST_TOOLCHAIN_FILE := rust-toolchain.toml

export MOLD_VERSION_FILE MOLD_SHA256SUMS_FILE RUST_TOOLCHAIN_FILE

check:
	@echo checking
