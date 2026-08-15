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

Path and standard-input collection share one private source-adapter
bounded-read helper. It accepts an inclusive 16 MiB and probes for one further
byte; excess input becomes `SourceReadError::TooLarge` and the stable
`source-too-large` operation. The helper may be called only by `read_path` and
`read_stdin`. It is not a domain port, a public stream utility, or permission
to add other input modes.

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

`AssignmentOperator` is the shared, closed domain and parser-port
representation for schema-v1 variable operators. The parser adapter is its only
producer; `SyntaxObservation` and report types are its permitted consumers. Its
`Define` variant serializes as an empty string and means a definition without
an assignment token: either a `define` block or a bare `export` directive, told
apart by the `define_block` flag. Extend the enum only through a
schema-versioned contract decision, and do not pass upstream operator strings
beyond the adapter.

The private `makefile_export` helpers own translation of operator-less export
definitions into variable observations. Before extraction, a repository sweep
found no equivalent directive-expansion helper: `variable_observation` was the
sole variable translator and produced one observation. In production,
`variable_observation` is the only permitted caller of `assignment_operator` and
`export_directive_observations`; `export_directive_observations` alone may call
`directive_names`. Focused unit tests may exercise each helper directly.
Compose the helpers only while translating one upstream `VariableDefinition`:
use `assignment_operator` for ordinary variable facts, and use
`export_directive_observations` only for an operator-less export so it can emit
zero or more directive facts with the shared directive span. They are
adapter-private mechanics, not domain ports, general directive parsers, or
reusable CST walkers.

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

`TraversalEvent` and `schedule_conditional` are private makefile-adapter
mechanics for the iterative CST walk. They may be used only by `collect_items`
to preserve source order while mutating one conditional-ancestry vector. Do not
expose them through the parser port or reuse them as a general tree walker.

The CLI adapter's private extraction, report-production, and report-emission
helpers divide its orchestration into focused steps. They may be called only by
the CLI adapter and must remain ordinary private functions. Promote one to a
port only if a distinct external capability needs the same contract, not merely
to share implementation detail or simplify a test.

The private `escape_control_characters` helper is restricted to fatal stderr
details. It preserves printable Unicode and escapes controls, so one diagnostic
cannot forge another physical line; it is not a general path normalizer or JSON
encoder.

The exact 0.3.40 parser requirement is temporarily patched to immutable fork
commit `2ae7134beb04416851ab18c8a5d5893348fbe26c`, which adds `!=` lexer
support and retains every name of a multi-name `export A B C` directive inside
the definition node. Keep the commit pin reproducible. When upgrading to an
upstream release that contains both fixes, remove the `[patch.crates-io]` entry
and rerun the complete assignment-operator contract matrix and the
export-directive suite before updating the lockfile.

Tests keep raw Makefile text under `tests/fixtures/makefiles/`. Unit and
property tests exercise the domain, `rstest-bdd` scenarios exercise observable
behaviour, black-box tests spawn the binary, and `insta` plus the JSON Schema
freeze the integration contract. The exact-byte multiline `define` fixture is
marked `-diff` in `.gitattributes` because its trailing whitespace is test
data; parser tests must continue to assert those bytes explicitly.

Cargo's default `serde_json` feature forwards to `ortho_config/serde_json`. Keep
`ortho_config` configured with `default-features = false` so no-default builds
do not enable its JSON integration implicitly. The direct `serde_json`
dependency remains the report serialization implementation and is not a
substitute for forwarding the OrthoConfig feature.

## Local Workflow

Use `make all` as the public entrypoint for formatting, linting, and tests.
`make lint` runs rustdoc, Clippy, and Whitaker. `make test` prefers
`cargo nextest run` and falls back to `cargo test` when cargo-nextest is not
available. `make audit` derives the Rust workspace root with `cargo metadata`,
logs workspace member manifests, and runs `cargo audit` once from the workspace
root. `make coverage` uses `cargo llvm-cov` with `lld`. Run
`make validate-makefile` whenever `Makefile` changes; the target invokes
`mbake validate Makefile` as the repository's Makefile validation entrypoint.

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

Install `clang`, `lld`, `mold`, `python3`, `cargo-audit`, and `mbake` before
running the full generated workflow locally on Linux. Install `mbake` with:

```shell
uv tool install mbake
```

## Spelling policy

Markdown uses en-GB-oxendict spelling enforced by the pinned `typos` release.
The tracked `typos.toml` is generated from the project-owned dictionary in
`data/typos-oxendict-base.toml` and the narrow repository overlay in
`typos.local.toml`. Run `make spelling` to refresh the ignored local base cache
when the tracked source is newer, regenerate the configuration, and check
maintained prose.

`make provenance` rejects personal repository references, local paths, named
operational projects, and claims of validation that cannot be reproduced from
the repository. `make markdownlint` includes this check. Generic consumer
contracts, technical dependency coordinates, and canonical citations in the
imported upstream guides remain permitted.

### Security audit ignores

Security audit jobs may set `CARGO_AUDIT_IGNORES` for narrowly scoped RustSec
advisories that affect unused or tooling-only dependency paths. Keep each
ignore tied to a documented runtime impact analysis, and remove it when the
affected dependency leaves the graph or the project starts using the advised
runtime path.
