# An empty conditional assignment exported by name on the next line,
# followed by rules carrying help comments.
CRATE ?= example
FORMAL_STUB ?= ./scripts/formal-stub.sh
FORMAL_STRICT ?=
export FORMAL_STRICT

build: target/debug/lib$(CRATE).rlib ## Build debug binary
release: target/release/lib$(CRATE).rlib ## Build release binary

all: release ## Default target builds release binary
