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

The private `ensure_round_trip` helper owns the concrete adapter's
byte-for-byte CST invariant. It may be called only by
`MakefileLosslessParser::parse`; it is not a domain policy, parser port, or
general text-comparison utility.

`SourceReader` is the source adapter's narrow capability interface for opening
one requested UTF-8 path. `read_path` owns complete byte collection and stable
`SourceReadError` classification, but must never call `ambient_authority`
itself. The CLI boundary constructs `AmbientSourceReader` once in `run_from`
and bundles it with process streams in `ProcessCapabilities`; tests and
embedded callers may instead use `run_from_with_reader`. Do not use
`SourceReader` for stdin, directory traversal, include expansion, parsing, or
general filesystem access, and do not promote it into the domain-owned parser
port.

Integration tests share `MockSourceReader` from `tests/common/mod.rs`, where
`mockall` remains a development-only dependency. Include
`tests/common/failing_reader.rs` only in suites that exercise post-open read
failures; do not compile shared test helpers into binaries that do not use
them, and do not suppress the resulting unused-code warnings.

`ConditionKind` is the shared, closed domain and parser-port representation for
`ifdef`, `ifndef`, `ifeq`, and `ifneq`. The parser adapter is its only producer;
`SyntaxObservation` and report types are its permitted consumers. Extend the
enum only when the supported GNU Make contract adds another directive, and do
not pass upstream strings beyond the adapter.

The makefile adapter privately scans leading recipe modifiers. This scanner
exists because the upstream API has no always-execute accessor and its silent
and ignore-error accessors are sensitive to modifier order. It may be called
only while translating an upstream recipe into a `RecipeObservation`; it is not
a general Make lexer, domain helper, or reusable port.

`rule_observation`, `variable_observation`, and `include_observation` are
private makefile-adapter constructors called only by `collect_items`. They keep
upstream field validation and source-span mapping beside CST translation. They
are not domain ports or general utilities; reuse outside `collect_items`
requires a new adapter-owned call-site with the same complete-observation
contract, not a move into the domain or ports modules.

The CLI adapter's private extraction, report-production, and report-emission
helpers divide its orchestration into focused steps. They may be called only by
the CLI adapter and must remain ordinary private functions. Promote one to a
port only if a distinct external capability needs the same contract, not merely
to share implementation detail or simplify a test.

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
