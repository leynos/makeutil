# Reduced from an external Makefile: a read-time guard written as a bare
# $(error ...) function directive inside a conditional block. GNU Make 4.4.1
# expands $(error ...) to empty text when the guard does not fire, so the
# line defines nothing and the parse is `complete`.
VERSION ?=
ifeq ($(strip $(VERSION)),)
$(error VERSION is empty; set version in Cargo.toml or pass VERSION explicitly)
endif

build:
	cargo build
