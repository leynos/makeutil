# Repository layout

This document describes the generated makeutil repository layout. It is the
canonical reference for where source code, tests, configuration, automation,
and long-lived documentation belong.

## Top-level tree

The tree below shows the generated repository structure. It is intentionally
compact and omits build output such as `target/`.

```plaintext
.
├── .cargo/
│   └── config.toml
├── .github/
│   ├── dependabot.yml
│   └── workflows/
│       ├── act-validation.yml
│       ├── ci.yml

│       └── release.yml

├── data/
│   └── typos-oxendict-base.toml
├── docs/
│   ├── adrs/
│   │   └── 0001-single-file-gnu-make-parse.md
│   ├── execplans/
│   │   └── adr-0001-single-file-gnu-make-parse.md
│   ├── contents.md
│   ├── design.md
│   ├── developers-guide.md
│   ├── ortho-config-users-guide.md
│   ├── polonius.md
│   ├── repository-layout.md
│   ├── terms-of-reference.md
│   ├── users-guide.md
│   └── ...
├── schemas/
│   └── makeutil.parse.v1.schema.json
├── src/
│   ├── adapters/
│   ├── domain/
│   ├── application.rs
│   ├── lib.rs
│   ├── main.rs
│   └── ports.rs
├── tests/
│   ├── features/
│   ├── fixtures/
│   ├── snapshots/
│   └── *.rs
├── AGENTS.md
├── Cargo.toml
├── LICENSE
├── Makefile
├── README.md
├── clippy.toml
├── codecov.yml
└── rust-toolchain.toml
```

## Path responsibilities

- `.cargo/config.toml`: Configures Cargo defaults for local development,
  including Linux linker and code-generation settings.
- `.github/dependabot.yml`: Configures automated dependency update checks.
- `.github/workflows/act-validation.yml`: Runs the generated workflow
  validation through `act` separately from main CI.
- `.github/workflows/ci.yml`: Runs the generated project's continuous
  integration checks.

- `.github/workflows/release.yml`: Builds and publishes binary release
  artefacts for the application flavour.

- `data/typos-oxendict-base.toml`: Owns the project-wide Oxford English
  dictionary used to generate `typos.toml`.
- `docs/`: Holds long-lived reference documentation, guides, style rules, and
  design material.
- `docs/adrs/`: Holds sequential, stable records of architectural decisions.
- `docs/adrs/0001-single-file-gnu-make-parse.md`: Records the boundary, accepted
  on 2026-07-13, for parsing one GNU Makefile into versioned JSON facts.
- `docs/execplans/`: Holds living, approval-gated implementation plans.
- `docs/execplans/adr-0001-single-file-gnu-make-parse.md`: Plans the staged,
  test-first implementation of ADR-0001.
- `docs/contents.md`: Indexes the documentation set and should be updated when
  documentation files are added, renamed, or removed.
- `docs/design.md`: Defines the living technical design, including the command
  contract, data model, architecture, and verification strategy.
- `docs/users-guide.md`: Explains `makeutil parse`, its JSON report, and exit
  contract.
- `docs/developers-guide.md`: Explains the contributor workflow and local
  tooling used to work on the generated project.
- `docs/polonius.md`: Records the Polonius compiler contract, borrow-centric
  design rules, and audited migration sites.
- `docs/ortho-config-users-guide.md`: Imports the command-line and configuration
  library guidance used to plan the CLI adapter.
- `docs/repository-layout.md`: Documents the repository tree and path
  responsibilities.
- `docs/terms-of-reference.md`: Defines the problem space, stakeholders, scope,
  constraints, and success criteria that govern the design.

- `schemas/`: Holds normative, versioned external JSON contracts.
- `src/adapters/`: Implements CLI, source, and GNU Make parser edges.
- `src/domain/`: Owns the stable report and source-location model.
- `src/application.rs`: Validates source and assembles reports through the
  parser port.
- `src/ports.rs`: Owns the minimal parser boundary and syntax observations.
- `src/lib.rs`: Exposes library application logic and doctested examples.
- `src/main.rs`: Contains the application entrypoint and top-level executable
  wiring.

- `tests/`: Holds unit-style integration, behavioural, schema, snapshot, and
  black-box process tests.
- `tests/features/`: Holds Gherkin acceptance specifications.
- `tests/fixtures/makefiles/`: Holds source-faithful parser inputs.
- `tests/snapshots/`: Holds reviewed JSON snapshots.
- `AGENTS.md`: Provides repository-specific working instructions for agents and
  contributors.
- `Cargo.toml`: Defines package metadata, dependencies, lint policy, and Cargo
  configuration.
- `LICENSE`: Records the project licence text.
- `Makefile`: Provides the public build, lint, test, coverage, and
  documentation validation commands.
- `README.md`: Introduces the project and gives the shortest useful
  getting-started path.
- `clippy.toml`: Configures Clippy lint behaviour that is not expressed
  directly in `Cargo.toml`.
- `codecov.yml`: Configures coverage reporting behaviour.
- `rust-toolchain.toml`: Pins the Rust toolchain channel and required
  components.

## Ownership boundaries

- Keep generated source code under `src/`. Add modules below `src/` when a
  feature grows beyond a small entrypoint or crate root.
- Keep black-box integration tests and externally observable workflow tests
  under `tests/`.
- Keep reusable documentation under `docs/`. Update `docs/contents.md` whenever
  a documentation file is added, renamed, or removed.
- Keep accepted and proposed architectural decisions under `docs/adrs/`; do not
  renumber a decision record after publication.
- Keep living implementation plans under `docs/execplans/` and preserve their
  approval, progress, decision, and retrospective sections as work proceeds.
- Keep build and validation entrypoints in `Makefile`; prefer adding or
  extending a Make target over documenting an ad hoc command.
- Keep continuous integration workflow changes under `.github/workflows/` and
  dependency-update policy under `.github/dependabot.yml`.
- Do not commit generated build output such as `target/`, coverage artefacts,
  or local editor state.

## Updating this document

Update this document when the repository gains a new top-level directory, a new
long-lived documentation category, a new workflow file, or a changed ownership
boundary that would otherwise make the tree misleading.
