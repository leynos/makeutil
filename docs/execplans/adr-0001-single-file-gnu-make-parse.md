# Implement single-file GNU Make parsing

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
and `Outcomes & retrospective` must be kept up to date as work proceeds.

Status: IMPLEMENTATION COMPLETE / AWAITING PR REVIEW.

## Approval

- Approver: User.
- Approval date: 2026-07-13.
- Exact `makefile-lossless = "=0.3.40"` exception: Approved.

When approval is granted, record the approver, date, exact-pin decision, and
change `Status` to `APPROVED` before beginning Milestone 1. Silence or approval
of a different document does not satisfy this gate.

## Purpose / big picture

Implement [ADR-0001](../adrs/0001-single-file-gnu-make-parse.md) so an operator
can run `makeutil parse PATH`, or provide one Makefile on standard input, and
receive one deterministic, versioned JSON document containing source-faithful
GNU Make facts. Malformed but recoverable source must still produce facts and
diagnostics with exit code 1; invocation, input, encoding, reporting, and
internal failures must use exit code 2. Source text must remain inert: parsing
must never invoke GNU Make, a shell, a recipe, a Make function, or an included
file.

Success is observable by running the compiled binary against complete,
recoverable, non-UTF-8, and hostile fixtures and observing the documented JSON,
streams, exit codes, byte-for-byte repeatability, and absence of sentinel side
effects.

## Constraints

- Do not begin implementation until the user explicitly approves this plan.
- Implement only the single-file GNU Make parse slice in ADR-0001. Do not add
  discovery, include traversal, evaluation, policy decisions, mutation,
  language bindings, batch processing, daemon behaviour, or another Make
  dialect.
- Treat the JSON schema version, not Rust types or the upstream concrete syntax
  tree (CST), as the stable integration contract. No Rowan or
  `makefile-lossless` type may cross into the domain model or public JSON.
- Parse source without invoking GNU Make, a shell, or any source-selected
  command. Open only the caller-supplied input path; report includes without
  opening them.
- Preserve exact input bytes through a complete upstream lossless round trip.
  Reject non-UTF-8 input before parsing.
- Keep byte ranges zero-based and end-exclusive. Keep displayed lines and byte
  columns one-based, including for multibyte UTF-8 and CRLF input.
- Preserve source order, use fixed serialized field order, append exactly one
  newline to compact JSON, and exclude host-dependent or time-dependent data.
- Use `cap_std`, `cap_std::fs_utf8`, and `camino` at filesystem boundaries in
  place of `std::fs` and `std::path`.
- Compile and test with the pinned nightly and Polonius. Preserve direct,
  borrow-centric forms described in [Polonius migration](../polonius.md).
- Every Rust module must begin with a `//!` comment, every public API must have
  Rustdoc with a useful example, and no Rust source file may exceed 400 lines.
- Follow Red-Green-Refactor for every behaviour milestone. The red command must
  fail for the expected missing behaviour before production code is added.
- Use `rstest` for fixtures and parameterized tests, `rstest-bdd` version
  `0.6.0-beta3` for behavioural scenarios, `googletest` matchers for semantic
  assertions, and `pretty_assertions` for equality failures where a structural
  diff is clearer. Use `insta` for stable multi-variant JSON output.
- Tests must not mutate the environment of the shared test process. Set
  environment variables only on child processes if a case requires them.
- Run `make check-fmt`, `make typecheck`, `make lint`, and `make test` after
  every major milestone, then have a scrutineer run
  `coderabbit review --agent`. Resolve all deterministic gate failures before
  requesting CodeRabbit. Resolve applicable CodeRabbit concerns as a separate
  review action before proceeding.
- Commit each accepted milestone atomically after its gates and review pass.
- Update user-facing behaviour in [the user's guide](../users-guide.md),
  internal interfaces and ownership in [the design](../design.md), repository
  paths in [the repository layout](../repository-layout.md), and contributor
  practices in [the developer's guide](../developers-guide.md).

The ADR and design require the exceptional exact requirement
`makefile-lossless = "=0.3.40"`, while repository policy normally mandates
caret requirements. Approval of this plan approves that narrow, documented
exception for this dependency only. If approval does not include the exception,
stop and resolve the conflict before editing `Cargo.toml`.

## Tolerances (exception triggers)

- Scope: stop if the implementation needs more than 35 new or modified tracked
  files, excluding fixture and snapshot files, or more than 2,500 net lines of
  Rust. Present a smaller decomposition before proceeding.
- Contract: stop if a required JSON field, location convention, stream rule, or
  exit code must differ from ADR-0001 or `docs/design.md`.
- Dependencies: stop before adding a runtime dependency not named in
  `Interfaces and dependencies`, or changing the exact upstream parser version.
- Upstream API: stop if `makefile-lossless` 0.3.40 cannot expose a required fact
  or source range without patching/forking it, or cannot losslessly retain a
  recovered tree.
- Input semantics: stop if implementation cannot preserve the exact
  caller-supplied UTF-8 path spelling, or the exact `--stdin-filename` value,
  without canonicalization or lexical cleaning.
- Testability: stop if a fatal or recovered path can be tested only by panic,
  shared-process environment mutation, or lint suppression.
- Quality: after three unsuccessful attempts to fix the same deterministic
  gate or CodeRabbit blocker, record evidence and escalate rather than masking
  it.
- Performance: stop if a 10 MiB fixture takes over two seconds or a 256-level
  conditional fixture uses more than 256 MiB resident memory on the development
  machine in three consecutive release-mode measurements. These are guardrails,
  not a public performance guarantee.
- Ambiguity: stop when two reasonable interpretations would produce different
  schema-v1 JSON or externally observable CLI behaviour.

## Risks

- Risk: upstream 0.3.40 accessors may not expose every location or conditional
  detail in the desired shape. Severity: high. Likelihood: medium. Mitigation:
  begin with a disposable, additive contract spike using the exact crate
  version and representative fixtures; promote only proven API mappings.
- Risk: an upstream recovered parse may expose facts differently from a
  complete `FromStr` parse. Severity: high. Likelihood: medium. Mitigation: use
  the upstream `Parse` result and its ordinary and positioned errors directly;
  snapshot both complete and recovered reports.
- Risk: upstream node ranges may not align with the construct ownership now
  fixed in `docs/design.md` section 6.2. Severity: high. Likelihood: medium.
  Mitigation: adapter contract tests compare every upstream range with the
  exact expected source slice. Derive a makeutil-owned range only from child
  ranges when the result is identical to the approved contract; otherwise stop.
- Risk: the ADR's exact pin conflicts with the repository-wide caret policy.
  Severity: medium. Likelihood: certain. Mitigation: treat explicit plan
  approval as approval of one scoped exception, document it in the design and
  developer guide, and do not generalize it.
- Risk: a schema represented only by Rust structs and snapshots is difficult
  for Concordat to validate independently. Severity: medium. Likelihood:
  medium. Mitigation: add a checked JSON Schema artefact for schema version 1
  and test representative complete and recovered documents against it.
- Risk: tests might claim “no execution” while only testing ordinary recipes.
  Severity: high. Likelihood: medium. Mitigation: end-to-end hostile fixtures
  contain `$(shell ...)`, `!=`, recipe commands, dynamic includes, and literal
  includes that would create a sentinel if evaluated or opened. Assert the
  sentinel remains absent.
- Risk: the Concordat integration criterion is outside this repository.
  Severity: medium. Likelihood: high. Mitigation: provide a consumer-shaped
  deserialization fixture and record the external Concordat trial as evidence
  required before implementation is declared complete; do not fabricate
  cross-repository proof. Outcome: the trial passed in the available Concordat
  checkout.
- Risk: strict lints and code-size limits may encourage premature abstraction.
  Severity: medium. Likelihood: medium. Mitigation: keep modules cohesive,
  sweep for equivalent helpers before every extraction, and add a trait only at
  the volatile parser boundary or when a deterministic failure path requires
  injection.

## Progress

- [x] (2026-07-13) Created the Leta workspace and mapped the scaffold, ADR,
  design, documentation, test guidance, and build gates with a Wyvern team.
- [x] (2026-07-13) Confirmed upstream `makefile-lossless` 0.3.40 exposes a
  lossless tree, recovered results, and ordinary and positioned diagnostics.
- [x] (2026-07-13) Imported the OrthoConfig user's guide from
  `../../ortho-config/docs/users-guide.md` and indexed it.
- [x] (2026-07-13) Completed the Logisphere community review and revised the
  design to freeze logical-path spelling, construct ranges, ordinal ownership,
  diagnostics, failure output, and observability before approval.
- [x] (2026-07-13) Passed all planning milestone deterministic gates and
  resolved every actionable concern from three CodeRabbit review rounds.
- [x] (2026-07-13) Obtained a clean CodeRabbit follow-up after the service rate
  limit reset; the final pre-completion review examined 34 files and reported
  zero findings.
- [x] (2026-07-13) Obtained explicit approval of this ExecPlan, including the
  exact parser pin exception and schema/path decisions.
- [x] (2026-07-13) Milestone 1: proved upstream contracts and froze the
  makeutil-owned domain and schema-v1 boundaries with red tests.
- [x] (2026-07-13) Milestone 2: implemented parser traversal, locations, and
  recovered output against the fixture corpus.
- [x] (2026-07-13) Milestone 3: implemented OrthoConfig CLI, source, JSON, and
  process adapters with behavioural and end-to-end validation.
- [x] (2026-07-13) Fixed `!=` lexing on fork branch
  `fix-shell-assignment-operator`, validated its 472 unit tests and 98
  doctests, and pinned immutable commit
  `8dd35801b75b332c2ac2f995ae398ef8238559fa` through `[patch.crates-io]`.
- [x] (2026-07-13) Passed the complete deterministic makeutil gate set after
  applying the patch; the scrutineer independently repeated every gate and
  CodeRabbit completed with zero findings across 34 reviewed files.
- [x] (2026-07-13) Added a consumer-owned schema-v1 deserialization test with
  focused red/green and Clippy evidence.
- [x] (2026-07-13) Completed the manual CLI acceptance exercise with path,
  recovered, and stdin exit codes `0`, `1`, and `0` respectively.
- [x] (2026-07-13) Measured exact-size 1, 5, and 10 MiB inputs and 256 nested
  conditionals in release mode; every run remained inside the elapsed-time and
  memory guardrails.
- [x] (2026-07-13) Ran the release binary from the Concordat Python 3.13
  environment, decoded schema v1 without a Rust binding, and found its required
  `build`, `lint`, and `test` targets in a complete parse.
- [x] (2026-07-13) Used `strace` to prove that existing literal and dynamic
  include paths were reported but never opened.
- [x] (2026-07-13) Milestone 4: synchronized contracts, completed all acceptance
  exercises, and passed every deterministic gate under independent scrutineer
  validation.
- [x] (2026-07-14) Reviewed the terminal diff and applied valid fixes for
  trailing variable whitespace, recipe-modifier ordering, closed conditional
  kinds, focused CLI helpers, and documentation drift. The focused whitespace
  and modifier-order tests supplied red evidence. The complete deterministic
  gate set then passed, and the scrutineer independently confirmed 45 of 45
  tests, two passing doctests with one intentionally ignored, and clean
  formatting, Polonius type-checking, lint, documentation, diagram, and diff
  checks.
- [x] (2026-07-14) Injected ambient filesystem access at the CLI composition
  boundary. Red compilation proved the `SourceReader` and
  `run_from_with_reader` seams were absent; focused source-adapter, output-
  failure, and BDD tests passed. The terminal repository gates then passed 49
  of 49 tests, two doctests with one intentionally ignored, and clean
  formatting, Polonius type-checking, rustdoc, Clippy, Whitaker, Markdown,
  spelling, Mermaid, and diff checks.
- [x] (2026-07-14) Corrected documentation ownership and orientation drift and
  applied the valid fatal CLI helper and private `collect_items` constructor
  fixes found during terminal review. The independent scrutineer confirmed 49
  of 49 tests, two passing doctests with one intentionally ignored, and clean
  `make check-fmt`, `make typecheck`, `make lint`, `make test`, rustdoc,
  Clippy, Whitaker, Markdown, spelling, Mermaid, and diff checks.
- [ ] Obtain CodeRabbit certification of the exact terminal diff through the
  pull request. The user approved deferral from the unavailable CLI review
  during CodeRabbit's temporary outage.

## Surprises & discoveries

- Observation: at planning start, the repository contained only greeting and
  test stubs; there was no parser, CLI, model, I/O, or test abstraction to
  extend. Evidence: Leta found only `greet`, `main`, and
  `replace_this_stub_when_real_tests_exist`; `Cargo.toml` had no dependencies.
  Impact: this was the first real application boundary, but it needed to remain
  a small set of cohesive modules rather than receive a framework-sized layout.
- Observation: upstream 0.3.40 exports `Parse`, `PositionedParseError`,
  `MakefileVariant`, and lossless CST types and retains parser errors alongside
  a tree. Evidence: the 0.3.40 docs.rs and tagged source expose these APIs and
  upstream tests assert recovered-tree and hostile-input round trips. Impact:
  an adapter can preserve partial evidence without exposing upstream types, but
  its contract must be proven before model implementation.
- Observation: the requested OrthoConfig guide originally did not exist in this
  checkout. Evidence: the source guide lived at
  `../../ortho-config/docs/users-guide.md`. Impact: it has been imported as
  `docs/ortho-config-users-guide.md` and is now a local implementation
  reference.
- Observation: `makefile-lossless` 0.3.40 documents `!=` as an assignment
  operator, but parses valid GNU Make `A != printf seven` as recovered rule
  fragments with diagnostics and exposes no `VariableDefinition`. Evidence: the
  focused `assignment_operators_remain_source_faithful::case_7` test and a live
  CLI reproduction both produce zero variable facts; the scrutineer
  independently reproduced the failure. Impact: this triggers the approved
  upstream stop condition. The exact pin cannot satisfy the source-faithful
  variable contract without an upstream fix, a separately approved narrow
  fallback parser, or an explicit scope reduction.
- Observation: the defect was confined to `Lexer::next_token`; the parser and
  AST accessors already recognized `!=`, but the operator-token start set
  omitted `!`. Evidence: fork commit `8dd35801b75b332c2ac2f995ae398ef8238559fa`
  changes that set and adds lexer and lossless AST regression tests. Impact:
  the existing adapter now reports shell assignments source-faithfully without
  a makeutil-specific parser fallback or vendored crate.
- Observation: the manual acceptance command named `complete.mk`, but the
  committed complete fixture is `all-facts.mk`. Evidence: the fixture corpus
  contains `all-facts.mk` and `recovered.mk`; the corrected command produced a
  complete schema-v1 report. Impact: the command below now uses the real
  fixture path.
- Observation: the consumer-shaped test initially expected an `all` target,
  while the representative fixture defines `check`. Evidence: the focused red
  run failed with a clear `check` versus `all` diff; changing only the consumer
  expectation made the focused test and Clippy pass. Impact: this supplies
  honest red/green evidence without changing production behaviour.
- Observation: focused review tests showed that trimming a variable value lost
  source-faithful trailing whitespace and that upstream recipe accessors did
  not recognize every ordering of leading `@`, `-`, and `+` modifiers. Evidence:
  `variable_values_preserve_trailing_whitespace` and
  `recipe_modifier_order_is_semantic` failed before their narrow adapter fixes.
  Impact: raw values now remain untrimmed, and one adapter-private scanner
  derives all three recipe flags without widening the parser port.
- Observation: a review finding claimed the file reader did not use a
  capability-oriented boundary, but `read_path` already used
  `cap_std::fs_utf8::File::open_ambient` with explicit ambient authority.
  Evidence: `src/adapters/source.rs` owns that call and maps its open and read
  failures into `SourceReadError`. Impact: the finding was stale and required
  no source-reader change at that review milestone. The user subsequently
  requested a stronger composition rule: ambient authority must be resolved
  only by the CLI and injected into `read_path`. Impact: the explicit new
  requirement supersedes the earlier no-change conclusion without changing
  error or CLI contracts.
- Observation: the claimed duplicate generated header in `typos.toml` was
  stale. Evidence: the file contains one two-line header emitted verbatim by
  `scripts/typos_rollout.py`. Impact: generated spelling policy required no
  manual edit.
- Observation: a tracing and metrics warning did not apply to this approved
  one-shot CLI slice. Evidence: stable operation identifiers are its documented
  observability surface, success and recovered parsing keep stderr empty, and
  the design explicitly defers metrics. Impact: no subscriber, recorder, or new
  telemetry dependency was added during terminal review.

## Decision log

- Decision: defer exact terminal-diff CodeRabbit certification to the pull
  request after the CLI first rate-limited and then required unavailable
  browser authentication during a temporary service outage. Rationale: the
  exact diff passed every independent deterministic gate, the immediately
  preceding review was clean, and the user explicitly approved waiting for PR
  review rather than blocking the commit. Date/Author: 2026-07-13 / User and
  Codex.

- Decision: generate large performance fixtures ephemerally and check their
  exact byte lengths with `stat` rather than commit 16 MiB of repetitive test
  data. Rationale: fixed `all:` and newline framing around repeated `a` bytes
  produces valid, deterministic rule fixtures while keeping the repository
  small; the measured command fails before timing if any size differs. Date/
  Author: 2026-07-13 / Codex.

- Decision: patch crates.io resolution to immutable fork commit
  `8dd35801b75b332c2ac2f995ae398ef8238559fa` while retaining the approved exact
  0.3.40 version requirement. Rationale: the minimal upstream-shaped fix adds
  the missing lexer start character and regression coverage without vendoring,
  changing makeutil policy, or exposing a mutable branch reference. Retire the
  patch when an adopted upstream release contains the fix. Date/Author:
  2026-07-13 / Codex.

- Decision: apply hexagonal architecture only at meaningful volatility and
  side-effect boundaries. Rationale: domain facts, locations, ordering, and
  parse outcome classification need pure tests; `makefile-lossless`, CLI
  parsing, filesystem access, and JSON output are adapters. Repositories, event
  buses, CQRS layers, and adapter-to- adapter traits would add ceremony without
  protecting a real boundary. Date/Author: 2026-07-13 / Codex planning team.
- Decision: define one domain-owned `MakefileParser` port and keep upstream CST
  observations on the adapter side. Rationale: the young parser crate is the
  principal volatile dependency. The port returns makeutil-owned facts and
  diagnostics so upstream APIs cannot leak into schema or application policy.
  Date/Author: 2026-07-13 / Codex planning team.
- Decision: use property testing for `LocationIndex`, not Kani or Verus.
  Rationale: arbitrary UTF-8, newline layouts, and valid byte spans form a
  natural generative invariant. There is no bounded concurrent/state machine
  model for Kani and no introduced lemma or contractual business theorem that
  would make a substantive Verus proof possible. Adding either would be
  performative rather than rigorous. Date/Author: 2026-07-13 / Codex planning
  team.
- Decision: provide JSON Schema Draft 2020-12 as a checked consumer artefact.
  Rationale: schema version 1 is the stable integration contract and must be
  independently machine-readable; Rust structs and snapshots alone are not an
  adequate subprocess contract. Date/Author: 2026-07-13 / Codex planning team.
- Decision: use OrthoConfig 0.8.x for the `parse` subcommand while keeping input
  selection explicit and unlayered. Rationale: the imported guide is the
  requested CLI/configuration reference, but ADR-0001 allows no implicit path
  or discovery. OrthoConfig supplies typed CLI derivation and preserves
  help/version display exits; it must not add environment or file defaults for
  `PATH` or `--stdin-filename`. Date/Author: 2026-07-13 / Codex planning team.
- Decision: preserve exact logical path spelling and use the complete
  construct-range rules in `docs/design.md` section 6.2. Rationale: callers
  need stable source slices and reproducible JSON. Deferring these choices
  until adapter implementation would make plan approval meaningless and
  accidentally turn upstream accessor choices into schema policy. Date/Author:
  2026-07-13 / Logisphere-reviewed Codex planning team.
- Decision: let the parser adapter return ordered makeutil-owned observations
  and source spans; keep round-trip bytes in adapter tests only. Rationale:
  location conversion, ordinals, and status are domain policy; exact-byte
  hashing is application-service policy. Upstream CST renderings and error
  types must not leak through the domain-owned port. Date/Author: 2026-07-13 /
  Logisphere-reviewed Codex planning team. Ownership wording clarified on
  2026-07-14 during terminal documentation review.
- Decision: serialize to memory before stdout and permit partial stdout only
  when the operating system accepts a prefix before an output failure.
  Rationale: the process can prevent serialization failures from writing JSON,
  but cannot retract accepted bytes after a broken pipe or partial write.
  Date/Author: 2026-07-13 / Logisphere-reviewed Codex planning team.
- Decision: keep review-driven helpers at their narrowest validated ownership
  boundary. `ConditionKind` is the closed domain/port representation consumed
  by observations and reports; the makefile adapter alone owns the private
  leading-recipe-modifier scanner; and CLI extraction, production, and emission
  helpers remain private to the CLI adapter. Rationale: these boundaries remove
  stringly typed drift and order-sensitive defects without creating reusable
  ports for implementation details. Permitted call sites and reuse policy are
  recorded in `docs/developers-guide.md`. Date/Author: 2026-07-14 / Wyvern
  review team.
- Decision: define `SourceReader` in the source adapter as a narrow capability
  interface, not a domain port. `read_path` owns byte collection and
  `SourceReadError` classification; `run_from` alone constructs the
  ambient-backed implementation, while `run_from_with_reader` supports tests
  and embedded composition through one `ProcessCapabilities` value. Rationale:
  this removes ambient authority from the reusable read function without
  transplanting filesystem concerns into the domain, introducing directory/
  include semantics, or exceeding the repository's four-argument limit. Date/
  Author: 2026-07-14 / Codex.
- Decision: do not derive an automatic `MakefileParser` mock for integration
  tests. Rationale: a `cfg_attr(test, automock)` mock is not exported when the
  library is compiled as a dependency of an integration-test crate. Exporting
  it would require production `mockall` or a public test-support feature and
  API solely for test ceremony; the existing small manual fake exercises the
  port without widening production dependencies or surface. Date/Author:
  2026-07-14 / Wyvern review team.

## Outcomes & retrospective

The implementation now exposes the approved single-file parse contract through
a capability-safe CLI and stable schema-v1 JSON. Unit, property, snapshot, BDD,
and end-to-end tests cover complete, recovered, fatal, and inert-source paths.
The forked parser fix restores source-faithful `!=` assignments without a
makeutil-specific fallback. Manual CLI acceptance, release-mode guardrails, and
the Concordat subprocess and include-boundary trials all pass. Independent
scrutineer validation repeated every deterministic gate. The implementation of
ADR-0001's single-file GNU Make parse slice is complete. Ambient filesystem
authority is now composed once at the CLI boundary and injected through
`ProcessCapabilities`; fake readers prove the source-open and source-read
contracts without filesystem access. Exact terminal-diff CodeRabbit
certification is deferred to the pull request because the CLI service became
unavailable, as explicitly approved by the user.

## Context and orientation

The repository is a Rust 2024 application compiled on the pinned nightly with
Polonius. `src/domain/` owns schema-v1 report types and source-location policy;
`src/ports.rs` owns the parser contract; and `src/application.rs` validates,
hashes, and assembles one source report. `src/adapters/` contains the
makefile-lossless parser, injected source capability, and CLI/reporting edges.
`src/main.rs` is the composition root. Unit, property, schema, behavioural, and
end-to-end tests under `tests/` replace the original greeting stubs.

[ADR-0001](../adrs/0001-single-file-gnu-make-parse.md) governs scope and the
stable subprocess contract. [The technical design](../design.md) defines JSON
fields, source locations, conditional flattening, determinism, security, and
the fixture classes. [The terms of reference](../terms-of-reference.md) govern
the larger problem boundary. [The repository layout](../repository-layout.md)
governs path ownership.

Implementation must consult these practice guides at the relevant milestone:

- [Rust testing with `rstest` fixtures](../rust-testing-with-rstest-fixtures.md)
  for reusable and parameterized test setup.
- [`rstest-bdd` user's guide](../rstest-bdd-users-guide.md), specifically the
  version 0.6.0-beta3 dependency and `#[scenario]` model, for feature tests.
- [Reliable testing via dependency injection](../reliable-testing-in-rust-via-dependency-injection.md)
  for deterministic adapters and failure injection.
- [Rust doctest DRY guide](../rust-doctest-dry-guide.md) for public examples
  shared with ordinary tests.
- [OrthoConfig user's guide](../ortho-config-users-guide.md), especially
  “Subcommand configuration” and “Preserving `clap` display exits”, for the CLI
  adapter.
- [Polonius migration](../polonius.md) before adding clones or ownership
  workarounds.

The implementing agent must load the `leta` skill for semantic navigation, the
`rust-router` skill to select only a necessary Rust specialist, the
`hexagonal-architecture` skill for boundary checks, and the `execplans` skill
to keep this document current. Use `firecrawl-mcp` only when an upstream API,
format, or prior-art gap remains after local documentation and exact dependency
source inspection. Use the `logisphere-experts` community for design reviews.

The intended narrow dependency flow is:

_Figure 1: Composition and dependency flow for the first parse slice._

```plaintext
CLI adapter ──> composition root ──> source reader
                       │                  │
                       └────────┬─────────┘
                                v
                    parse application service
                                │
                                v
                    domain-owned parser port
                                │
                                v
                    makefile-lossless adapter

parse report ──> composition root ──> JSON reporter ──> stdout / process exit
```

The domain owns schema-v1 value types, the `SourceIdentity` contract, source
locations, conditional ancestry, global ordinal assignment, diagnostic order,
and complete/recovered classification. The application service validates one
source byte buffer as UTF-8, calculates its exact-byte SHA-256 digest, and
coordinates its logical path with the parser port. Adapters own
OrthoConfig/clap, capability-oriented file or stdin reading, upstream parsing
into ordered observations, Serde serialization, streams, and process exit.
Adapters never call each other; `src/main.rs` is the composition root.

## Plan of work

### Milestone 1: prove contracts and establish pure boundaries

Start with a repository-wide Leta and text sweep for existing helpers, ports,
models, and fixture conventions. Record in `docs/design.md` the bounded
context, the single parser port's ownership and permitted caller, adapter
composition, error mapping, exact-pin exception, logical-path rule, and
source-range ownership. If this work changes ADR-0001 rather than clarifying
it, create a new ADR in the documented style and reference it from the design;
never silently rewrite an accepted decision.

Add exact 0.3.40 to an adapter contract test and prove GNU variant selection,
complete and recovered trees, positioned diagnostics, every required accessor,
conditional branch traversal, range slices, and byte-for-byte render. Introduce
a compiling stub adapter first. The red test
`adapter_contract_reports_complete_rule_observations` must then fail an
assertion because the stub returns no observations, not because a symbol is
missing. If any required mapping is unavailable, stop under the upstream
tolerance before designing around it.

Before schema types are implemented, build a go/no-go range matrix in
`docs/design.md` and contract tests for the complete rule, targets and
prerequisites, recipe including its tab and modifiers, variable directive,
include directive, conditional and else directive, positioned diagnostic,
line-derived diagnostic, EOF, CRLF, and continued lines. Each case asserts the
exact source slice specified in design section 6.2. Approval freezes those
semantics; Milestone 1 verifies that the exact upstream version can implement
them.

Define a compact module layout, adjusting names only when the helper sweep
finds an existing convention:

- `src/domain.rs` and focused children own `ParseReport`, `ParseStatus`,
  `SourceLocation`, `ParseDiagnostic`, `RuleFact`, `RecipeFact`, `VariableFact`,
  `IncludeFact`, `ConditionContext`, and `LocationIndex`.
- `src/ports.rs` owns the minimal `MakefileParser` trait and makeutil-owned
  parser outcome/error types.
- `src/application.rs` owns `parse_source`, which accepts raw source bytes plus
  a logical source name and parser port, maps invalid bytes to `source-utf8`,
  and passes the validated `&str` to the parser port.
- `src/adapters/` owns the upstream parser, source input, CLI, and JSON
  reporter.

Do not create a file for each type. Keep cohesive types together and stay below
400 lines. Return semantic `thiserror` enums from library boundaries and format
stable fatal diagnostics explicitly in `main`; do not add an opaque error
dependency unless a later approved design decision establishes a concrete need.

Add red `rstest` cases for location indexing, schema serialization order,
complete/recovered classification, and fixed exit classification. Add
`proptest` invariants: every valid generated byte span maps monotonically,
round-trips its byte slice, uses one-based display positions, and never splits
UTF-8; EOF and CRLF positions remain defined. Implement the smallest pure model
to make them green, then refactor.

Add `schemas/makeutil.parse.v1.schema.json` using JSON Schema Draft 2020-12 and
tests that validate complete and recovered examples. Update `docs/design.md`
with the normative schema path. Define every required field, nullable field,
enum, integer minimum, the lower-case 64-hex SHA-256 pattern, fixed tool/parser
constants, array and diagnostic ordering, and always-emitted empty arrays. Apply
`additionalProperties: false` recursively. Self-validate the schema, validate
every snapshot, and reject malformed near-miss documents.

Run the four required gates. A scrutineer then runs CodeRabbit. Resolve all
concerns, update this ExecPlan's evidence and decisions, and commit the
milestone before proceeding.

### Milestone 2: collect source-faithful facts

Build `makefile-lossless` adapter traversal behind `MakefileParser`. Traverse
root items in source order and iteratively flatten conditional arms while
carrying outer-to-inner `ConditionContext`. Generate global ordinals only after
source ordering is unambiguous. Translate all upstream ranges and diagnostics
at the adapter boundary. Never follow include paths or evaluate expressions.

Create external fixtures under `tests/fixtures/makefiles/` for every class in
`docs/design.md` section 11.1. Use `rstest` parameterization rather than
copying test bodies. Cover multiple/repeated/double-colon rules, prerequisites,
continuations, all supported variable operators and flags, define blocks,
recipe prefixes, all four GNU conditional forms with `if` and `else` nesting,
literal/optional/dynamic includes, empty input, no trailing newline, CRLF,
multibyte UTF-8, recoverable syntax, large input, deep conditionals, and
hostile text.

Use `googletest` matchers for membership, order, option, and error semantics and
`pretty_assertions` for full structured fact comparisons. Add `insta`
snapshots for at least one complete document, one recovered document with
multiple diagnostics, one nested-conditional document, and one document
containing every fact variant. Keep raw fixture input external to Rust source
files.

For each family, run the focused test in red before its collector code, make
the minimal green change, and refactor only after the focused and wider adapter
suite pass. Round-trip every complete fixture through the exact upstream tree.
Recovered fixtures must always retain partial facts and classify as exit 1.

Run the four gates, then scrutineer CodeRabbit review, concern resolution,
ExecPlan update, and an atomic commit.

### Milestone 3: wire CLI, input, JSON, and process behaviour

Use OrthoConfig 0.8.x and its clap integration to define one `parse`
subcommand. It accepts exactly one UTF-8 path token or `-`; stdin requires
`--stdin-filename`. Propagate clap `ArgMatches` through OrthoConfig's
`with_matches` path, or the equivalent 0.8.0 API, so only explicitly supplied
CLI values populate these fields. Do not enable configuration-file discovery or
environment fallbacks for either input. Add omitted-PATH and omitted-
`--stdin-filename` tests alongside stdin, help, and version cases. Keep
`src/main.rs` limited to composition, tracing initialization if diagnostics
need it, stream writes, and exit classification.

Freeze exit and error mapping before wiring: help and version display exit 0
using clap's normal display stream; usage errors, non-UTF-8 path arguments,
missing `--stdin-filename`, and conflicting stdin options use `cli` and exit 2;
open and read errors use `source-open` or `source-read` and exit 2; invalid
file bytes use `source-utf8` and exit 2; recovered parser diagnostics emit JSON
and exit 1; parser invariant failures use `parse-internal` and exit 2;
in-memory serialization uses `json-serialize` and exit 2; broken pipe or other
write failure uses `stdout-write` and exit 2. Panics are defects and are not
converted into stable diagnostics by a catch boundary.

Implement capability-oriented path reading with `cap_std::fs_utf8` and
`camino`; keep a narrow stdin reader. Calculate SHA-256 over exact bytes,
reject invalid UTF-8 before parsing, and retain the caller-supplied logical
path without filesystem canonicalization. The JSON reporter writes one compact
document plus newline to stdout for complete and recovered results and writes
no progress prose. Fatal errors go to stderr. Serialization failures emit no
JSON; stdout-write failures may leave only the partial prefix described in
`docs/design.md` section 10.1.

Add `tests/features/parse.feature` and Rust step bindings using `rstest-bdd`
0.6.0-beta3. Keep this specification synchronized with the tests:

```gherkin
Feature: Parse one GNU Makefile into JSON facts

  Scenario: Parse a complete Makefile by path
    Given a complete GNU Makefile fixture
    When makeutil parses the fixture by path
    Then stdout contains one schema version 1 JSON document
    And the process exits with code 0
    And stderr is empty

  Scenario: Parse complete source from standard input
    Given complete GNU Makefile source on standard input
    When makeutil parses dash with stdin filename Makefile
    Then the report source path is Makefile
    And the process exits with code 0

  Scenario: Report a recovered parse
    Given a recoverable GNU Makefile fixture
    When makeutil parses the fixture by path
    Then stdout contains recovered facts and diagnostics
    And the process exits with code 1

  Scenario: Reject a missing input path
    Given a path that does not exist
    When makeutil attempts to parse the missing path
    Then stdout is empty
    And stderr reports the source-open operation
    And the process exits with code 2

  Scenario: Reject non UTF-8 source
    Given source bytes that are not valid UTF-8
    When makeutil attempts to parse those bytes
    Then stdout is empty
    And stderr reports the source-utf8 operation
    And the process exits with code 2

  Scenario: Keep source-selected commands inert
    Given a Makefile containing shell functions, recipes, assignments, and includes
    When makeutil parses the hostile fixture
    Then no sentinel side effect exists
    And the process emits only source facts
```

Add black-box end-to-end tests that spawn the built binary with child-process
stdin and environment only. Test help/version, missing or extra input, missing
stdin filename, nonexistent paths, a directory supplied as a file, non-UTF-8
file bytes, Unix non-UTF-8 path arguments, complete and recovered streams,
exact compact newline output, two byte-identical runs, and hostile source that
cannot create a sentinel. Prove include non-traversal separately with
`strace -f -e trace=openat,openat2` and assert that existing literal and
dynamic include paths never appear in file-open calls.

Serialize the report fully into memory before output. Use an injected writer
seam to test a failure before any byte and a failure after a short partial
write. Broken pipe and all stdout write failures use operation identifier
`stdout-write` and exit 2; partial stdout is permitted only after an output
write failure. Use an injected reader for permission and mid-read failures,
rather than an unreliable unreadable-file E2E under privileged CI.

Delete `greet`, the greeting `main`, its lint exception, and `tests/stub.rs`
only after replacement tests are green. Run the release-mode large/deep input
guardrail, the four gates, scrutineer CodeRabbit review, concern resolution, a
clean follow-up review, ExecPlan update, and an atomic commit.

### Milestone 4: synchronize contracts and prove acceptance

Rewrite `docs/users-guide.md` around `makeutil parse`: path and stdin examples,
field meanings, byte locations, stdout/stderr, exit 0/1/2, recovery,
unsupported scope, deterministic output, and the inert-input security
guarantee. Update `docs/contents.md` to index every ExecPlan and long-lived
document added by the work. Update `docs/developers-guide.md` with module
ownership, port/adapter rules, helper reuse policy, fixtures, snapshots, exact
parser upgrade gate, and the test-first workflow. Update
`docs/repository-layout.md` for source modules, `schemas/`, features, fixtures,
snapshots, and end-to-end tests. Reconcile ADR-0001 with the documentation
style guide and confirm that its Accepted status is supported by current
external evidence.

Add a consumer-shaped test that deserializes representative schema-v1 JSON
without linking Rust implementation types. Record the command and result for an
actual Concordat subprocess trial when that repository is available. If it is
not available, leave the ADR Proposed and record the external gap.

Run `make fmt` after documentation changes, followed by `make markdownlint` and
`make nixie`. If the Makefile changes, also run `mbake validate Makefile`. Then
run the four required gates. Scrutineer runs the final CodeRabbit review; clear
all concerns, update this plan and its retrospective, and commit. Do not mark
the plan COMPLETE until every acceptance criterion has current evidence.

## Concrete steps

Resolve and enter the repository root before running commands:

```shell
cd "$(git rev-parse --show-toplevel)"
```

At the start of each implementation session, confirm scope and status:

```shell
git status --short --branch
leta workspace add "$PWD"
leta files
```

For a focused Red-Green-Refactor cycle, replace the filters below with the new
test's actual module and scenario names and record exact output in `Progress`:

```shell
RUSTFLAGS="-Zpolonius=next -D warnings" cargo test location_index --all-features
RUSTFLAGS="-Zpolonius=next -D warnings" cargo test parser_adapter --all-features
RUSTFLAGS="-Zpolonius=next -D warnings" cargo test --test parse_bdd --all-features
RUSTFLAGS="-Zpolonius=next -D warnings" cargo test --test parse_cli --all-features
```

The red run must fail because the new behaviour is absent, not because the test
does not compile for an unrelated reason. The corresponding green run must pass
without ignored or expected-failure markers.

After every major milestone, run deterministic gates in this order:

```shell
make check-fmt
make typecheck
make lint
make test
```

Expected successful endings include no warnings and exit status 0. Only after
all four pass may the scrutineer run:

```shell
coderabbit review --agent
```

Resolve every applicable concern, rerun affected focused tests and all four
gates, rerun CodeRabbit to obtain a clean follow-up, update this document, then
commit the milestone. Never commit with a failing gate. Within a milestone,
make reviewable checkpoint commits after domain/schema, upstream contract,
rules/recipes, variables/includes/conditions, CLI/source, reporter/process, and
BDD/E2E units become independently green. Run CodeRabbit at the major milestone
boundary rather than on every checkpoint.

For the documentation milestone, run:

```shell
make fmt
make markdownlint
make nixie
```

If the milestone changes `Makefile`, also run `mbake validate Makefile`.

The final manual acceptance exercise is:

```shell
cargo build --bin makeutil
target/debug/makeutil parse tests/fixtures/makefiles/all-facts.mk
target/debug/makeutil parse tests/fixtures/makefiles/recovered.mk
printf 'all:\n\t@echo ok\n' | target/debug/makeutil parse --stdin-filename Makefile -
```

The first parse command and the stdin pipeline must emit one compact schema-v1
JSON line and exit 0. The recovered-fixture command must emit recovered facts
and diagnostics and exit 1. Capture exit codes explicitly during implementation
rather than relying on a shell pipeline that hides them.

Generate deterministic 1 MiB, 5 MiB, and 10 MiB valid rule fixtures with the
ephemeral generator recorded in the Decision log, assert their exact lengths
before measuring, build release mode, warm each input once, then measure three
runs with `/usr/bin/time -v`. Record median elapsed time and maximum resident
set size in `Artefacts and notes`. Require the 10 MiB median to remain under
two seconds and maximum resident set size under 256 MiB on Linux, and inspect
the three sizes for super-linear growth. Exercise the generated 256-level
conditional fixture in three consecutive release-mode measurements with
`/usr/bin/time -v`; require each Linux run to stay under 256 MiB maximum
resident set size and prove that iterative traversal does not overflow the
stack. On a non-Linux host, record that RSS is not comparable and retain
elapsed-time and correctness evidence.

## Validation and acceptance

Acceptance requires all ADR criteria plus the following evidence:

- Unit tests prove every fact type, condition ancestry, ordinal ordering,
  location edge case, classification, and error mapping.
- Property tests prove location monotonicity, valid end-exclusive ranges,
  one-based display locations, and UTF-8 byte-column behaviour across generated
  inputs.
- Adapter tests prove exact 0.3.40 mappings and byte-for-byte complete-tree
  round trips for the full fixture corpus.
- `insta` snapshots and JSON Schema validation freeze complete, recovered,
  nested, and all-variant JSON documents.
- BDD scenarios prove the user language of path, stdin, complete, recovered,
  fatal, and inert parsing.
- End-to-end tests prove real binary arguments, stdin, streams, exits, trailing
  newline, repeat determinism, and hostile source with no sentinel effect.
- A 10 MiB fixture and 256 nested conditionals remain inside the tolerance
  guardrail without stack failure or uncontrolled allocation.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass at every
  milestone and at final acceptance.
- `make markdownlint` and `make nixie` pass for documentation;
  `mbake validate Makefile` passes if the Makefile changes.
- CodeRabbit reports no unresolved applicable concerns after deterministic
  gates.
- A consumer-shaped JSON contract test passes. Actual Concordat subprocess
  evidence is recorded before claiming cross-repository integration or moving
  the ADR to Accepted.

Red-Green-Refactor evidence must be appended to `Progress` for each milestone:
the exact red command and expected failure, the green command and pass, and the
post-refactor focused and wider gate results.

## Idempotence and recovery

Tests, formatters, schema validation, and documentation gates are repeatable.
Fixture and snapshot updates must be reviewed as contract changes, not accepted
blindly. `insta` pending files are diagnostic artefacts; inspect them, accept
only intended schema changes, and remove stale pending files before committing.

If a milestone fails halfway, retain its red/green evidence in `Progress`, use
`git diff` and focused tests to identify the incomplete unit, and resume from
the last passing atomic commit. Do not use destructive Git commands. If an
adapter spike fails its go/no-go criterion, delete only the additive spike in a
separate reviewed change or retain it as documented evidence; do not conceal
the upstream limitation.

Begin each milestone from a clean checkpoint or record exactly which reviewed,
uncommitted changes belong to it. If review fixes are intentionally
uncommitted, rerun their focused tests before resuming. Remove stale
`.snap.new` files only after comparing them with the approved schema; never
bulk-accept snapshots.

## Artefacts and notes

Firecrawl research used the authoritative 0.3.40 docs.rs source and tagged
upstream repository. It confirmed that the crate exports its GNU Make variant,
lossless `Makefile`, parse-result type, ordinary errors, positioned errors,
rules, recipes, variables, includes, conditionals, and Rowan ranges. Milestone
1 compile-checked those mappings against the exact dependency.

The Wyvern team independently found no existing abstraction to reuse and
recommended the same narrow parser-port boundary. The community-of-experts
review and scrutineer evidence must be appended here before this draft is
offered for approval.

The scrutineer recorded passing `git diff --check`, Markdown and spelling,
Nixie, Rust formatting, Polonius type-checking, rustdoc, Clippy, Whitaker,
nextest, and doctest gates. Three completed CodeRabbit rounds reported 11, 9,
and 7 actionable concerns respectively; all were addressed. A later
pre-completion review completed successfully across 34 files with zero findings.

The final manual CLI exercise produced `complete=0`, `recovered=1`, and
`stdin=0`. Every command wrote one schema-v1 JSON document, no command wrote to
standard error, and the reports classified their parse status as expected.

The include-boundary exercise created existing `literal.mk` and `dynamic.mk`
files next to the input, traced `openat` and `openat2`, and asserted that
neither include path occurred in the syscall log. The binary reported both
includes in a complete parse; the result was `include_opened=false`.

Release-mode `/usr/bin/time -v` evidence after one warm-up per input was:

| Input                   | Elapsed runs           | Maximum RSS runs (KiB) |
| ----------------------- | ---------------------- | ---------------------- |
| 1 MiB                   | 0.01 s, 0.01 s, 0.01 s | 7,168; 7,232; 7,316    |
| 5 MiB                   | 0.06 s, 0.06 s, 0.06 s | 23,608; 23,624; 23,708 |
| 10 MiB                  | 0.12 s, 0.12 s, 0.12 s | 44,100; 44,380; 44,080 |
| 256 nested conditionals | 0.01 s, 0.01 s, 0.01 s | 8,808; 8,672; 8,852    |

The large inputs were exact-size single rules generated from a fixed `all:`
header, repeated `a` bytes, and a newline. A `stat` assertion checked every
length before timing. The nested input contained 256 deterministic `ifdef`/
`endif` pairs around one rule. Growth was sub-linear across the measured sizes,
the 10 MiB median was 0.12 seconds, and all resident-set measurements were
below 256 MiB.

From `/data/leynos/Projects/concordat`, a Python 3.13 subprocess invoked the
release binary against Concordat's Makefile, decoded JSON with the standard
library, asserted schema version 1 and complete status, and found `build`,
`lint`, and `test`. The successful summary was:

```plaintext
{"schema_version":1,"status":"complete","required_targets":["build","lint","test"],"language_binding":false}
```

## Interfaces and dependencies

The domain-facing shape should remain close to the following; exact fields must
match `docs/design.md` and the JSON Schema:

```rust
pub trait MakefileParser {
    fn parse(&self, source: &str) -> Result<ParserOutcome, ParserPortError>;
}

pub fn parse_source(
    source: &[u8],
    logical_path: &str,
    parser: &impl MakefileParser,
) -> Result<ParseReport, ParseApplicationError>;
```

`SyntaxObservation` is makeutil-owned and carries one ordered syntax fact or
diagnostic with byte spans but no calculated line/column, ordinal, status,
upstream error, Rowan node, or rendered CST. The adapter enforces round-trip in
its contract tests. `parse_source` validates the bytes as UTF-8, hashes exact
bytes, calls the port with `&str`, and assigns locations, ordinals, diagnostic
order, and complete/recovered status.

Planned runtime dependencies are:

- `makefile-lossless = "=0.3.40"`, the explicitly approved exact exception;
- `ortho_config = "0.8.0"` for typed CLI derivation and display exits;
- `serde = "1.0.228"` with `derive` and `serde_json = "1.0.150"` for the owned
  report contract;
- `sha2 = "0.11.0"` for exact-byte source identity;
- `camino = "1.2.4"`, `cap-std = "4.0.2"` with `fs_utf8`, and
  `thiserror = "2.0.18"` for path, filesystem, and semantic error boundaries.

Before adding each non-exception dependency, verify its current compatible
caret version and smallest necessary feature set. Preserve the approved exact
`makefile-lossless = "=0.3.40"` requirement unchanged. Resolve it through the
temporary full-SHA fork patch recorded above until upstream contains the fix.
Do not add both a direct `clap` dependency and OrthoConfig's re-exported
surface unless the derive/API contract requires it.

Planned development dependencies are `rstest = "0.26.1"`,
`rstest-bdd = "0.6.0-beta3"`, `rstest-bdd-macros = "0.6.0-beta3"`,
`googletest = "0.14.3"`, `pretty_assertions = "1.4.1"`, `insta = "1.48.0"` with
JSON support, `proptest = "1.11.0"`, `jsonschema = "0.47.0"` with default
features disabled, `assert_cmd = "2.2.2"`, and `tempfile = "3.27.0"`. The
child-process tests must inspect raw exit codes and independent stdout/stderr.
Record selected features and the resolved `Cargo.lock` versions in milestone
evidence. No Kani or Verus dependency is planned for the reasons in the
Decision log.

## Revision note

Initially revised 2026-07-13 after Wyvern, Logisphere, and CodeRabbit review to
freeze path, range, schema, parser-port, failure-output, CLI merge, security,
performance, and dependency decisions and to import and correct the OrthoConfig
0.8.0 guide. Implementation completed on 2026-07-13 with deterministic gates,
manual acceptance, performance measurements, and external Concordat and
include-boundary evidence recorded above. Pull request review remains pending.
Revised again on 2026-07-14 to inject the ambient filesystem capability at the
CLI boundary while preserving the stable source error and process diagnostic
contracts. Terminal documentation review then clarified hashing ownership and
replaced planning-time scaffold descriptions in the current repository
orientation and applied the valid CLI and parser-helper fixes. Independent
scrutineer validation passed all post-correction gates; exact terminal-diff
CodeRabbit certification remains pending in the pull request.
