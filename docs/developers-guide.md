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
an assignment token: a `define` block, or a bare `export` or `unexport`
directive. The `define_block` flag tells a block from a directive, and
`exported` then tells `export` from `unexport`. Extend the enum only through a
schema-versioned contract decision, and do not pass upstream operator strings
beyond the adapter.

The private `makefile_export` helpers own translation of operator-less `export`
and `unexport` definitions into variable observations. `OperatorContext`
records which directive keyword a line carries. On a directive line a leading
`unexport` decides, so `is_exported` is false for every fact it yields. An
assignment is a separate case: there `export` is a modifier wherever it appears
among the prefixes, so `variable_observation` reads `is_export` for it, and
`unexport export FOO = 3` is exported, as GNU Make exports it. Before
extraction, a repository sweep found no equivalent directive-expansion helper:
`variable_observation` was the sole variable translator and produced one
observation. In production, `variable_observation` is the only permitted caller
of `assignment_operator` and `export_directive_observations`;
`export_directive_observations` alone may call `directive_names`. Focused unit
tests may exercise each helper directly. Compose the helpers only while
translating one upstream `VariableDefinition`: use `assignment_operator` for
ordinary variable facts, and use `export_directive_observations` only for an
operator-less `export` or `unexport` so it can emit zero or more directive
facts with the shared directive span. They are adapter-private mechanics, not
domain ports, general directive parsers, or reusable CST walkers.

The private `makefile_expansion` module owns the reading of bare expansion
lines, the parser's `MakefileItem::Expansion` items (see
[ADR-0003](adrs/0003-bare-expansion-lines.md)). A repository sweep found no
existing helper that judges an expansion: the adapter had no expansion item to
translate before the fork added one. `expansion_observation` returns nothing
for a line of `info`, `warning` or `error` calls, which GNU Make expands to
empty text. For any other line it returns one diagnostic spanning the line
without its newline. `collect_items` is its only production caller, once per
expansion item. Keep the empty-expansion set to functions that GNU Make
documents as expanding to empty text, and change it only with ADR-0003, since
adding a name turns `recovered` reports `complete`. It is adapter-private, not
a general expansion evaluator.

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
commit `4f4463b261d949c16c7d7f28785f74c9f45badea`, protected by the annotated
tag `makeutil-pin-4f4463b`. It adds `!=` lexer support, retains every name of a
multi-name `export A B C` directive inside the definition node, parses
`unexport` as a directive beside `export`, and keeps a bare expansion line as
its own item. Keep the commit pin reproducible, and protect any new pin with a
`makeutil-pin-<sha>` tag before repinning. When upgrading to an upstream
release that contains these fixes, remove the `[patch.crates-io]` entry and
rerun the complete assignment-operator contract matrix and the export-directive
suite before updating the lockfile.

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

The test suite runs once per pull request, in `ci.yml`'s coverage step. That
step runs the same tests `make test` runs except the doctests, which
`build-test` runs in a step of its own with
`cargo test --doc --workspace --all-features`. The repository used to carry an
`act-validation.yml` workflow that ran `make test WITH_ACT=1`, but nothing reads
`WITH_ACT` and no test is gated on Act, so that workflow ran the whole suite a
second time and was removed. The crate's only feature, `serde_json`, is in
`default`, so `make test`'s `--all-features` selects the same tests as the
coverage run's default. `tests/workflow_suite_contract.rs` holds the split.

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

## The build standard

Development, test, lint, and typecheck builds use the parallel `rustc` frontend
(`-Zthreads=8`) and, on Linux, the `mold` linker (`-Clink-arg=-fuse-ld=mold`).
These are defaults in `.cargo/config.toml`, which Cargo discovers on its own,
so a bare `cargo build` gets them. `mold` ships for Linux only, so the linker
flag lives in a Linux-only table and macOS and Windows keep their platform
linker. Cargo selects one `rustflags` source rather than merging them, so every
source repeats the same flags apart from the linker.

An assigned `RUSTFLAGS` replaces the configuration's flags, so the Makefile
recipes that set it compose the standard's flags onto any inherited value (CI's
`setup-rust` exports one). Two builds are deliberately excluded: coverage
assigns `RUSTFLAGS` without the fast flags, because a measurement should not
depend on them, and the release recipe and workflow keep the platform linker,
because they assign `RUSTFLAGS` (even an empty value displaces the
configuration). Cargo has no per-profile `rustflags`, so a direct
`cargo build --release` takes the configuration's flags unless `RUSTFLAGS` is
assigned too.

On Linux, install `mold` before building: the configuration names it, so a
build without it fails at link time. CI installs it through `setup-rust`'s
`install-mold` input. `tests/build_standard_contract.rs` holds the standard. It
reads the configuration sources, the commands `make -n` prints for each
development target on a Linux host and a macOS host (each keeping the caller's
own `RUSTFLAGS`) and for each coverage and release target on a Linux host, and
the `setup-rust` steps of the CI workflows (each must pass `install-mold`), so
a flag lost through a recipe or workflow edit fails there.
`tests/ci_step_env_contract.rs` holds the builds that assign their own
environment. The coverage recipe (read from `make -n coverage`) and the CI
coverage step link with `lld` through `clang`, because LLVM coverage tools
expect LLVM-compatible linker behaviour, and the recipe selects the LLVM
backend; the doctest step restates the flags `make test` gives its doctest
line; and the release build step replaces the repository's `rustflags`, whose
Linux entry links with mold, with `-Zpolonius=next` alone so the static musl
binary links with the runner's default. Each step is judged by its own `env:`
block, so a sibling step's environment, a comment or an inline comment neither
supplies nor hides a value.

CI also installs `clang` and `lld` through `setup-rust`'s `install-clang-lld`
input, which installs both on Linux and fails the job unless `clang` and
`ld.lld` resolve on `PATH`; the workflows carry no hand-rolled `apt-get` step.
Both inputs skip with a notice on other platforms and set no linker flag, so
`.cargo/config.toml` and the coverage step's environment still choose which
linker runs. `tests/linker_provisioning_contract.rs` reads the parsed
`setup-rust` step of each CI workflow and asserts both inputs are `'true'` and
that no step installs a linker by hand.

### Cranelift

Cranelift is the development-profile codegen backend. The full suite was
measured under it on the pinned `nightly-2026-05-28` on 2026-09-28: all 174
nextest tests and the doctests pass. Coverage selects LLVM explicitly
(`CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm`), because instrumentation needs it,
and release builds use the release profile, which Cranelift does not touch.
Re-measure the whole suite on the next toolchain bump; if it fails, record the
failing tests here as an exception and remove the backend from
`.cargo/config.toml`.

## Releases

`.github/workflows/release.yml` publishes a release when a tag of the form
`v<major>.<minor>.<patch>` is pushed. The tag must equal `v` followed by the
`version` in `Cargo.toml`; the build job checks this and stops otherwise. The
workflow builds two statically linked musl binaries, each natively on a runner
of its own architecture:

- `x86_64-unknown-linux-musl`, on `ubuntu-latest`;
- `aarch64-unknown-linux-musl`, on `ubuntu-24.04-arm`.

It builds with the nightly channel pinned in `rust-toolchain.toml` and
`RUSTFLAGS=-Zpolonius=next`, because the crate does not compile on stable. It
smoke-tests each binary by parsing a fixture and checking the report's schema
version, status, exported variables and rules. The release job then attaches
each binary and its `.sha256` file to a GitHub release with generated notes, and
`verify-binstall` installs the published release through cargo-binstall on
both glibc triples.

A pull request that edits `release.yml` runs the build and smoke-test jobs
without publishing. It also exercises the tag check against the crate's own tag
and a mismatching one, and the `release-dry-run` job downloads the artefacts as
the release job does and requires exactly the four expected files. Read that
run before tagging: a broken release workflow shows there rather than on the
tag.

## Spelling policy

Markdown uses en-GB-oxendict spelling. Run `make spelling` to enforce it;
`make markdownlint` runs the same gate. The gate regenerates the tracked
`typos.toml` from the live shared dictionary and the narrow repository overlay
in `typos.local.toml` on every run, so a word added to the shared dictionary
needs no change here.

`TYPOS_CONFIG_BUILDER_VERSION` in the `Makefile` pins the
`typos-config-builder` release the gate runs (currently `v0.1.3`). Raise it
together with the regenerated `typos.toml`, never on its own. The builder
requires Python 3.14 or newer, so the target passes `--python 3.14` and `uv`
fetches that interpreter when the host lacks one.

Because the dictionary is live, `typos.toml` must never be drift checked in
continuous integration; it is generated output and hand edits are overwritten
on the next run. Add narrow repository-specific identifier, API, proper-name,
or fixture exceptions to `typos.local.toml`.

`make provenance` rejects personal repository references, local paths, named
operational projects, and claims of validation that cannot be reproduced from
the repository. `make markdownlint` includes this check, and the CI
`build-test` job runs it as a step of its own, which
`tests/workflow_suite_contract.rs` holds. Generic consumer contracts, technical
dependency coordinates, and canonical citations in the imported upstream guides
remain permitted.

A GitHub Actions coordinate, `<owner>/<repository>/.github/actions/<name>@`, is
a technical dependency coordinate: a workflow contract must name the action it
asserts. The personal-repository check therefore skips each such occurrence and
still rejects every other reference to the owner's repositories, including a
repository, an issue, a URL, or a reusable-workflow path, even on the same line
as a coordinate. The exemption uses a Perl-compatible lookahead, so the check
needs a Git built with PCRE support; without it, `git grep -P` fails and the
check fails with it. The reference after the `@` may be empty, because a
workflow contract names the action as a prefix that ends at the `@`.
`tests/provenance_target.rs` runs the real recipe in a throwaway Git repository
for accepted coordinates, each rejected shape, and generated coordinates and
stray references. It builds the owner name at run time so that the file does
not trip the check it tests.

### Security audit ignores

Security audit jobs may set `CARGO_AUDIT_IGNORES` for narrowly scoped RustSec
advisories that affect unused or tooling-only dependency paths. Keep each
ignore tied to a documented runtime impact analysis, and remove it when the
affected dependency leaves the graph or the project starts using the advised
runtime path.

## Markdown formatting

Markdown follows the estate's `markdown-formatting-baseline` rule.

- `make fmt` rewrites Markdown with
  `mdtablefix --in-place --git --include-untracked --wrap --renumber --breaks
  --ellipsis --fences`,
  then runs `markdownlint-cli2 --fix "**/*.md"`.
- `make check-fmt` runs the same mdtablefix command with `--check` in place of
  `--in-place`, and fails when any file would change.
- `--git --include-untracked` selects the Markdown files Git tracks plus the
  untracked files Git does not ignore, so a new document is checked before it
  is staged.
- `.markdownlint-cli2.jsonc` carries the canonical markdownlint configuration.
  Keep its `config` entries and `ignores` globs; add repository-specific rules
  or globs beside them.
- CI installs mdtablefix 0.6.1 with the shared `install-mdtablefix` action
  before `make check-fmt`, and lints Markdown with
  `DavidAnson/markdownlint-cli2-action` over `**/*.md`.

Install mdtablefix 0.6.1 or later locally with
`cargo binstall --no-confirm mdtablefix@0.6.1`, or
`cargo install --locked mdtablefix@0.6.1`. Install markdownlint-cli2 with
`bun add --global markdownlint-cli2` or
`npm install --global markdownlint-cli2`.

Three groups of tests hold this wiring:

- `tests/workflow_suite/markdown_wiring.rs` reads the Makefile with makeutil's
  own parser and requires each recipe's flags and exit status, including
  `markdownlint-cli2 --fix` in `make fmt`. A flag or tool that appears only in
  a shell comment does not count.
- `tests/workflow_suite/markdown_ci_wiring.rs` reads the workflows by their
  indentation structure, whatever its width. It requires an installer step at
  mdtablefix 0.6.1 or later before `make check-fmt` in the same job, a
  `globs: '**/*.md'` input under the lint action's own `with:`, and the
  canonical rule settings in `.markdownlint-cli2.jsonc`.
- `tests/markdown_formatting_targets.rs` runs the real `make fmt` and
  `make check-fmt` in a scratch Git repository with recording stubs for `cargo`,
  `mdtablefix` and `markdownlint-cli2`. It asserts the arguments, the order,
  and that a failing tool fails the target.
