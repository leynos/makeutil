# Two conditional assignments, then two consecutive bare exports naming
# them, a double blank line, and a rule.
#
# Both are exported rather than assigned on the recipe's command line.
PG_PASSWORD ?= embedded_test
POSTGRESQL_RELEASES_URL ?= https://github.com/theseus-rs/postgresql-binaries
export PG_PASSWORD
export POSTGRESQL_RELEASES_URL


# Zero-tolerance documentation gate.
docs-check: deps
	bun run docs:check
