# Developer Guide

This guide explains the contributor workflow and internal conventions for
makeutil.

The normative architecture is in [the design](design.md), with the accepted
slice boundary in [ADR-0001](adrs/0001-single-file-gnu-make-parse.md) and paths
described by [the repository layout](repository-layout.md).

## Parser boundary

`MakefileParser` is the only parser port. Its implementation returns ordered,
makeutil-owned `SyntaxObservation` values; upstream CST nodes and errors must
not cross the adapter boundary. `parse_source` owns UTF-8 validation, hashing,
locations, global ordinals, and complete-versus-recovered classification.

New syntax collection belongs in the existing parser adapter unless a distinct
external capability requires another port. CLI path and stdin filename values
must continue to use OrthoConfig's explicit `ArgMatches` extraction, without
file or environment layers.

The exact 0.3.40 parser requirement is temporarily patched to immutable fork
commit `8dd35801b75b332c2ac2f995ae398ef8238559fa`, which adds `!=` lexer
support. Keep the commit pin reproducible. When upgrading to an upstream
release that contains the fix, remove the `[patch.crates-io]` entry and rerun
the complete assignment-operator contract matrix before updating the lockfile.

Tests keep raw Makefile text under `tests/fixtures/makefiles/`. Unit and
property tests exercise the domain, `rstest-bdd` scenarios exercise observable
behaviour, black-box tests spawn the binary, and `insta` plus the JSON Schema
freeze the integration contract.

## Local Workflow

Use `make all` as the public entrypoint for formatting, linting, and tests.
`make lint` runs rustdoc, Clippy, and Whitaker. `make test` prefers
`cargo nextest run` and falls back to `cargo test` when cargo-nextest is not
available. `make audit` derives the Rust workspace root with `cargo metadata`,
logs workspace member manifests, and runs `cargo audit` once from the workspace
root. `make coverage` uses `cargo llvm-cov` with `lld`.

GitHub Actions Act validation lives in `.github/workflows/act-validation.yml`.
The main `.github/workflows/ci.yml` workflow deliberately does not run
`make test WITH_ACT=1`; the separate Act workflow runs those slower
container-backed checks in parallel.

## Tooling

Development builds use Cranelift for debug code generation. On Linux targets,
`.cargo/config.toml` configures clang to link with `mold` so debug builds link
quickly. Coverage generation uses `lld` because LLVM coverage tooling expects
LLVM-compatible linker behaviour.

The project compiles with the Polonius borrow-checking analysis on the pinned
nightly toolchain. `.cargo/config.toml` supplies `-Zpolonius=next` to Cargo and
rust-analyzer; Makefile and coverage commands that override `RUSTFLAGS` include
the flag explicitly. Use the pinned toolchain through plain `cargo` commands,
not an unpinned `cargo +nightly`, because the development profile also requires
the pinned Cranelift component. See [Polonius migration](polonius.md) before
introducing borrow-checker workarounds.

Install `clang`, `lld`, `mold`, `python3`, and `cargo-audit` before running the
full generated workflow locally on Linux.

## Spelling policy

Markdown uses en-GB-oxendict spelling enforced by the pinned `typos` release.
The tracked `typos.toml` is generated from the estate-wide shared dictionary
and the narrow repository overlay in `typos.local.toml`. Run `make spelling` to
refresh the ignored local shared-base cache when the published source is newer,
regenerate the tracked configuration, and check maintained prose.

### Security audit ignores

Security audit jobs may set `CARGO_AUDIT_IGNORES` for narrowly scoped RustSec
advisories that affect unused or tooling-only dependency paths. Keep each
ignore tied to a documented runtime impact analysis, and remove it when the
affected dependency leaves the graph or the project starts using the advised
runtime path.
