# makeutil

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](
https://deepwiki.com/leynos/makeutil)

*Parse one GNU Makefile into deterministic, source-faithful JSON.*

`makeutil` reports rules, recipes, variables, includes, conditionals, source
locations, and recoverable diagnostics without evaluating the Makefile or
following its includes.

______________________________________________________________________

## Quick start

Parse the repository's representative fixture:

```shell
cargo run -- parse tests/fixtures/makefiles/all-facts.mk
```

The command writes one compact JSON document to standard output.

______________________________________________________________________

## Development

This crate requires the Polonius borrow-checking analysis (`-Zpolonius=next`)
on the pinned nightly toolchain. The flag is configured in
`.cargo/config.toml`; see [Polonius migration](docs/polonius.md) for the
project's borrow-checking conventions and audit inventory.

______________________________________________________________________

## Documentation

- [Documentation contents](docs/contents.md)
- [User guide](docs/users-guide.md)
- [Developer guide](docs/developers-guide.md)
