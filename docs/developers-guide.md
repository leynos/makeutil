# Developer Guide

This guide explains the contributor workflow for the generated
makeutil project.

## Local Workflow

Use `make all` as the public entrypoint for formatting, linting, and tests.
`make lint` runs rustdoc, Clippy, and Whitaker. `make test` prefers
`cargo nextest run` and falls back to `cargo test` when cargo-nextest is not
available. `make audit` derives the Rust workspace root with
`cargo metadata`, logs workspace member manifests, and runs `cargo audit` once
from the workspace root. `make coverage` uses `cargo llvm-cov` with `lld`.

GitHub Actions Act validation lives in `.github/workflows/act-validation.yml`.
The main `.github/workflows/ci.yml` workflow deliberately does not run
`make test WITH_ACT=1`; the separate Act workflow runs those slower
container-backed checks in parallel.

## Tooling

Development builds use Cranelift for debug code generation. On Linux targets,
`.cargo/config.toml` configures clang to link with `mold` so debug builds link
quickly. Coverage generation uses `lld` because LLVM coverage tooling expects
LLVM-compatible linker behaviour.

Install `clang`, `lld`, `mold`, `python3`, and `cargo-audit` before running the
full generated workflow locally on Linux.

## Spelling policy

Markdown uses en-GB-oxendict spelling enforced by the pinned `typos` release.
The tracked `typos.toml` is generated from the estate-wide shared dictionary
and the narrow repository overlay in `typos.local.toml`. Run `make spelling` to
refresh the ignored local shared-base cache when the published source is newer,
regenerate the tracked configuration, and check maintained prose.

### Security audit ignores

Security audit jobs may set `CARGO_AUDIT_IGNORES` for narrowly scoped
RustSec advisories that affect unused or tooling-only dependency paths. Keep
each ignore tied to a documented runtime impact analysis, and remove it when
the affected dependency leaves the graph or the project starts using the
advised runtime path.
