# Reduced from leynos/pg-embed-setup-unpriv: a read-time guard written as a
# bare $(error ...) function directive inside a conditional block. GNU Make
# accepts this, but makefile-lossless 0.3.40 (patched) cannot yet represent
# a bare function directive, so the parse must degrade to `recovered` rather
# than report a false `complete`.
VERSION ?=
ifeq ($(strip $(VERSION)),)
$(error VERSION is empty; set version in Cargo.toml or pass VERSION explicitly)
endif

build:
	cargo build
