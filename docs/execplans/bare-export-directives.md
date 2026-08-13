# Parse bare `export` and `unexport` directives without aborting

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: BLOCKED — Stages A to D, F and G are complete and merged into this
branch. Stage E cannot proceed under the delegated authority for this work
because it changes a separate repository; see the `Decision Log`. Everything
achievable without that change has been delivered.

## Purpose / big picture

`makeutil parse` reads one GNU Makefile and prints a versioned JSON document of
syntax facts. Today it aborts outright on a construct that appears in ordinary
real-world Makefiles: a bare `export` directive that exports variables which
were assigned elsewhere, with no assignment on the `export` line itself.

A one-line Makefile containing only `export FOO` produces no JSON at all. The
tool prints a fatal message on standard error and exits 2:

```plaintext
makeutil: parse-internal: required variable-assignment-operator accessor was absent
```

Because `makeutil parse` handles a whole file at once, a single such line
destroys the report for the entire Makefile. Every fact in the file — every
rule, every variable, every include — is lost, not just the export line.
Downstream tools that consume the JSON therefore record an operational error
for the whole repository rather than a set of facts with one gap.

After this change, a novice can run `makeutil parse` over a Makefile containing
`export MOLD_VERSION_FILE MOLD_SHA256SUMS_FILE RUST_TOOLCHAIN_FILE` and get a
complete JSON report on standard output with exit code 0, in which each
exported name appears in the `variables` array with an empty operator. No
construct in this plan may cause a `parse-internal` abort ever again.

Observable success, in one command:

```console
$ printf 'FOO := 1\nBAR := 2\nexport FOO BAR\n' > /tmp/demo.mk
$ makeutil parse /tmp/demo.mk | python3 -m json.tool | head -20
$ echo "exit=$?"
exit=0
```

The report must show `"status": "complete"` with an empty `diagnostics` array,
and `variables` must contain five entries: the two assignments and the three
directive facts described below.

## Constraints

These are hard invariants. If satisfying the objective requires violating one,
stop, record the conflict in `Decision Log`, and escalate rather than working
around it.

- The JSON contract is versioned and consumers pin `makeutil` by commit SHA.
  `schema_version` must remain `1` and the file
  `schemas/makeutil.parse.v1.schema.json` must keep
  `"additionalProperties": false` at every level. No new top-level key and no
  new property on the `variable` object may be introduced by this plan, because
  under `additionalProperties: false` such an addition is a breaking change for
  any consumer validating against version 1. See the `Decision Log` for the
  representation chosen to respect this.
- `parse.status` must remain honest. `complete` means the parser emitted no
  diagnostics and the facts are trustworthy; `recovered` means the tool could
  not fully understand the input. Never report `complete` for a construct the
  tool cannot faithfully represent, and never silently discard a construct
  while claiming `complete`. Consumers treat any status other than `complete`
  as indeterminate and fail closed, so a wrong `complete` is worse than a
  `recovered`.
- The byte-for-byte round-trip invariant in
  `ensure_round_trip` (`src/adapters/makefile.rs`, lines 46-52) must continue
  to hold for every fixture added by this plan.
- Process exit codes are part of the contract and must not change: 0 for a
  `complete` parse, 1 for a `recovered` parse, 2 for a fatal internal or
  input-handling failure. This mapping lives in `src/adapters/cli.rs` at line
  211 (`exit_code: u8::from(report.parse.status != ParseStatus::Complete)`) and
  line 197 (the `parse-internal` fatal path).
- Do not modify anything in the consumer repositories that pin `makeutil`.
  Re-pinning is their responsibility and is out of scope here.
- `make provenance` is a commit gate that rejects certain operator-specific and
  owner-qualified strings appearing anywhere outside the `Makefile` itself. Do
  not paste absolute filesystem paths from your working environment, nor
  fully-qualified forge URLs for the parser fork, into any Markdown or Rust
  file. Use repository-relative paths and bare revision hashes. If the gate
  fails, read the `provenance` target in `Makefile` to see exactly what it
  matched.
- Clippy lints in this repository are unusually strict (see `[lints.clippy]` in
  `Cargo.toml`): `unwrap_used`, `expect_used`, `indexing_slicing`,
  `option_if_let_else`, `must_use_candidate` and others are `deny`. Lints must
  not be silenced. In tests, `.expect(...)` is permitted; in production code
  and in non-`#[cfg(test)]` helpers it is not.
- No source file may exceed 400 lines. `src/adapters/makefile.rs` is currently
  325 lines, so there is limited headroom; extract helpers into a new module if
  the budget would be exceeded.

## Tolerances (exception triggers)

- Scope: if the makeutil-side change (Stages B to D) touches more than six files
  or more than 250 net lines, stop and escalate.
- Interface: if any public signature in `src/ports.rs`, `src/domain/mod.rs`, or
  `src/application.rs` must change shape, stop and escalate. Adding a variant
  to the private-facing `SyntaxObservation` enum is expected and is not an
  escalation; changing `ParseReport` is.
- Schema: if any change to `schemas/makeutil.parse.v1.schema.json` appears
  necessary, stop and escalate. That is a version-2 conversation, not this plan.
- Dependencies: if a new crate dependency is required, stop and escalate. The
  revision bump of the already-patched `makefile-lossless` in Stage E is not a
  new dependency and is expected.
- Upstream: Stage E changes a separate repository. If the upstream change
  requires touching the lexer (`src/lex.rs` upstream) rather than the parser
  (`src/lossless.rs` upstream), stop and escalate — that is a much larger blast
  radius than this plan assumes.
- Iterations: if a red test still fails after five green attempts, stop and
  escalate with the failing output.
- Time: if any single stage exceeds three hours, stop and escalate.

## Risks

- Risk: the upstream parser change in Stage E alters the concrete syntax tree
  shape for inputs unrelated to `export`, silently changing existing reports.
  Severity: high. Likelihood: low. Mitigation: the upstream change is gated
  behind "a directive prefix keyword was consumed on this line", which cannot
  be true for a plain assignment or a rule. Before bumping the pinned revision,
  run the full `make test` suite and compare the two `insta` snapshots in
  `tests/snapshots/` — an unexpected snapshot diff is the tripwire.
- Risk: representing a bare `export FOO` as an entry in the `variables` array
  misleads a consumer that reads `variables` as "the set of variables this
  Makefile assigns". Severity: medium. Likelihood: medium. Mitigation: the
  discriminating predicate is documented in `docs/users-guide.md` and in the
  schema-adjacent documentation as part of Stage D, and is asserted by a test.
  See the `Decision Log` entry on representation for the full trade-off.
- Risk: the pinned upstream revision cannot be rebuilt or the fork is
  unavailable when Stage E runs. Severity: medium. Likelihood: low. Mitigation:
  Stages B to D deliver standalone value (no more aborts) without touching
  upstream. Stage E is a separate, independently revertible commit. If upstream
  is unavailable, stop after Stage D and record the multi-name limitation as a
  known gap.
- Risk: `make lint` runs `cargo doc` with `-D warnings` and a third-party lint
  driver (`whitaker`) that may not be installed in every environment. Severity:
  low. Likelihood: medium. Mitigation: run
  `cargo clippy --all-targets --all-features -- -D warnings` directly as a
  fallback and record in `Surprises & Discoveries` that the full `make lint`
  could not be executed, rather than declaring the gate passed.

## Progress

- [x] Stage A: orientation and reproduction confirmed on the current working
      tree (no code changes). Every row of the behaviour table reproduced
      exactly; no divergence from the pin.
- [x] Stage B: red tests and fixtures added; each fails for the expected reason.
      Evidence in `Artefacts and notes`.
- [x] Stage C: minimal makeutil change so no `export` or `unexport` form aborts.
- [x] Stage D: refactor, documentation, snapshots, and full commit gates.
- [ ] Stage E: BLOCKED. The upstream parser fix lives in a separate repository
      that this work is not authorized to push to. See the `Decision Log`.
- [x] Stage F: `unexport` behaviour pinned by regression test
      (`unexport_directive_degrades_honestly` in `tests/corpus.rs`) and
      documented as a known limitation in `docs/users-guide.md`.
- [x] Stage G: consumer-facing note added as §6.6.2 of `docs/design.md`. Two
      deviations: it records the behaviour change rather than a new parser
      revision to pin, because Stage E did not run; and it names the merge of
      this branch rather than the merge commit SHA the plan asks for, which is
      unknowable before the merge happens. Whoever merges should substitute the
      SHA, or accept the branch reference as sufficient.

## Surprises & discoveries

Recorded during investigation, before implementation began.

- Observation: the upstream parser already models a single-name bare export
  correctly. `export FOO\n` produces a clean tree with no errors at all.
  Evidence: the concrete syntax tree for `export FOO\n` is
  `ROOT@0..11 > VARIABLE@0..11` containing `IDENTIFIER "export"`, `WHITESPACE`,
  `IDENTIFIER "FOO"`, `NEWLINE`, and `Parse::errors()` is empty. The accessor
  `VariableDefinition::name()` returns `Some("FOO")`, `is_export()` returns
  `true`, and `assignment_operator()` returns `None`. Impact: the single-name
  defect is entirely inside makeutil. It is fixed by Stage C alone, with no
  upstream work and no schema work.
- Observation: the multi-name form, which is the one that actually occurs in the
  wild, is *not* modelled upstream. Only the first name is captured; the second
  lands in an error node and the third escapes the variable node entirely.
  Evidence: for `export FOO BAR BAZ\n` the tree is `ROOT@0..19` containing
  `VARIABLE@0..14` (with `ERROR@11..14` wrapping `IDENTIFIER "BAR"`), then a
  loose `WHITESPACE`, a loose `IDENTIFIER "BAZ"`, and a `NEWLINE` as direct
  children of `ROOT`. One error is reported: `expected assignment operator` at
  range `10..11`. Impact: Stage C alone makes this input parse without
  aborting, but it reports `recovered` and loses two of the three names.
  Consumers that fail closed on `status != "complete"` are still blocked. Stage
  E is therefore required to actually resolve the real-world case, not optional
  polish.
- Observation: `unexport` is not recognized as a directive at all; it is parsed
  as a rule whose first target is the word `unexport`. Evidence:
  `makeutil parse` on a file containing `unexport FOO BAR\n` exits 1 and emits
  `"rules": [{"targets": ["unexport", "FOO", "BAR"], ...}]` with
  `"status": "recovered"` and the diagnostic `expected ':'`. The same holds for
  `unexport` alone. Impact: `unexport` never aborts, so it is not the blocking
  defect, but the facts it produces are actively misleading — a consumer sees a
  rule that does not exist. Faithfully modelling `unexport` needs a way to say
  "this name was explicitly un-exported", which schema version 1 cannot
  express. Stage F pins the current behaviour and documents the gap; full
  support is deferred.
- Observation (Stage C): adding the export handling inline took
  `src/adapters/makefile.rs` to 398 lines, two lines below the 400-line limit.
  Impact: the extraction the plan permits was taken rather than deferred. The
  operator mapping, the directive-name walk, and the directive expansion now
  live in `src/adapters/makefile_export.rs` (107 lines) with their unit tests in
  `src/adapters/makefile_export_tests.rs`, leaving `makefile.rs` at 318 lines.
  The unit tests named in Stage B therefore live in the new test file rather
  than in `src/adapters/makefile_tests.rs`.
- Observation (Stage C): upstream reports the `expected assignment operator`
  positioned diagnostic at a byte range relative to the offending line rather
  than to the file, so for a multi-line fixture the diagnostic location points
  at the wrong line. Evidence: for `FOO := 1\nBAR := 2\nexport FOO BAR\n` the
  positioned diagnostic is reported at bytes 3..4, which is on line 1, while the
  `export` is on line 3. Impact: pre-existing and outside this plan's scope.
  Stage E removes the diagnostic for this input entirely, so the mislocation
  stops being visible for exports, but it presumably remains for other
  recovered constructs. Worth its own investigation.
- Observation (stage review): `export unexport` is a legitimate, cleanly parsed
  Makefile line exporting a variable named `unexport`. Evidence: upstream
  reports `name = Some("unexport")`, `is_export() = true`, and no errors at
  all. Impact: any implementation that identifies directive keywords by token
  text discards it. The first implementation did exactly that and reported
  `complete` with the fact missing. See the `Decision Log`.
- Observation (stage review): `export define FOO ... endef` is modelled by
  upstream with `name() = None`, `is_export() = true` and `is_define() = true`,
  together with three errors. Evidence: the same holds for `export define` with
  no name. Impact: gating the no-facts path on "export and not define" left
  this form aborting with exit code 2. The plan's Stage C wording did not
  anticipate the two modifiers co-occurring.
- Observation (second stage review): upstream does not diagnose every export
  line it fails to name. Evidence: `override export override` parses with no
  errors at all and `name() = None`, because upstream's own `name()` accessor
  refuses any identifier whose text is `export`, `override` or `define`.
  Impact: the first fix for the name-less case returned no observations and
  trusted upstream to supply the diagnostic, so this input produced a
  `complete` report with the whole line silently missing — the same honesty
  breach the previous review had found in a different shape. The adapter now
  emits its own diagnostic, so the guarantee no longer depends on upstream's
  behaviour. This is the second time trusting an upstream invariant produced a
  false `complete`; prefer guarantees the adapter can enforce itself.
- Observation (second stage review): `override define FOO ... endef` still
  aborts with exit code 2 and the `variable-name` message, losing the whole
  file exactly as bare exports used to. Evidence: reproduced against the built
  binary. It is a documented GNU Make construct and is not an `export` form, so
  it is outside this plan's scope and was left alone rather than fixed
  opportunistically. Impact: undiscovered work. Like the target-specific export
  defect below, it deserves its own plan, and the two would sensibly be planned
  together as "definition forms that still abort".
- Observation: a target-specific export is silently modelled wrongly and, unlike
  the cases above, reports `complete`. Evidence: `foo: export BAR := baz\n`
  parses with `"status": "complete"` and a single rule whose prerequisites are
  `["export", "BAR", ":=", "baz"]`. Impact: this is a separate honesty defect
  outside the scope of this plan. Do not fix it here. Record it so it is not
  lost; it deserves its own plan.

## Decision log

- Decision: represent a bare `export NAME` as an entry in the existing
  `variables` array, using the schema's existing empty-string operator, rather
  than adding a new top-level `exports` array to the report. Rationale: three
  options were weighed. (1) A new top-level `exports` array is the most honest
  representation, but the schema sets `"additionalProperties": false` on the
  root object, so any consumer validating a report against
  `schemas/makeutil.parse.v1.schema.json` would reject reports containing it.
  That is a breaking change requiring a version-2 schema and a coordinated
  re-pin by every consumer — disproportionate to a crash fix. (2) Recording the
  directive only as a recoverable diagnostic would stop the abort but force
  `status: "recovered"`, and consumers fail closed on any status other than
  `complete`. Every Makefile with a bare export would remain effectively
  unparsable downstream. This fails the purpose of the work. (3) Reusing
  `variables` with `operator: ""` costs nothing in the schema: the `operator`
  enum already contains `""`, introduced for `define` blocks, and the existing
  `exported` and `define_block` booleans are enough to discriminate. The
  predicate a consumer applies is: `operator == "" && define_block == false`
  means "this is a bare export directive, not an assignment"; `raw_value` is
  the empty string for such a fact. This keeps `status: "complete"`, unblocks
  consumers immediately, and changes no schema file. The accepted cost is
  conflation: a naive consumer treating every entry in `variables` as an
  assignment will now see three extra "variables" for `export A B C`. This is
  mitigated by documentation in `docs/users-guide.md` and by a test that pins
  the discriminating predicate. It is recorded as a medium risk above.
  Date/Author: 2026-08-13, plan author.

- Decision: do not bump `schema_version`, and do not edit
  `schemas/makeutil.parse.v1.schema.json` at all. Rationale: follows directly
  from the representation decision. Existing reports keep validating; new
  reports validate against the unchanged version-1 schema because no new key or
  enum member is introduced. This is verified rather than assumed by extending
  the `reports_validate_against_schema` case list in `tests/report_schema.rs`.
  Date/Author: 2026-08-13, plan author.

- Decision: split the work into a makeutil-only fix (Stages B to D) and an
  upstream parser fix (Stage E), delivered as separate commits. Rationale: the
  single-name case and the never-abort guarantee are entirely within makeutil's
  control and deliver value immediately. The multi-name case requires the
  upstream tree to carry all the names, which makeutil cannot synthesize from a
  tree that has already discarded them. Splitting keeps the upstream revision
  bump independently revertible if it causes an unexpected snapshot change.
  Date/Author: 2026-08-13, plan author.

- Decision: makeutil, not the upstream crate, owns extraction of the exported
  name list from the syntax tree. Rationale: the upstream change should be
  confined to the parser's tree shape, keeping the upstream diff small and its
  existing accessors (`VariableDefinition::name()`, which returns the first
  name) backwards compatible. makeutil already imports `SyntaxKind` and
  `rowan::ast::AstNode` in `src/adapters/makefile.rs`, so walking the variable
  node's identifier tokens is a local, well-scoped helper rather than a new
  upstream API surface that must then be supported forever. Date/Author:
  2026-08-13, plan author.

- Decision: defer faithful `unexport` support and defer the target-specific
  export defect; both are documented rather than fixed. Rationale: `unexport`
  needs a representation for "explicitly un-exported", which schema version 1
  cannot express without a new field, and the constraint above forbids that.
  Neither construct causes an abort, so neither blocks the purpose of this
  plan. Pinning the current behaviour in the corpus suite makes the gap visible
  and makes any future upstream improvement fail loudly rather than change
  reports silently. Date/Author: 2026-08-13, plan author.

- Decision: `variable_observation` returns `Vec<SyntaxObservation>` rather than
  `Option<SyntaxObservation>`, as the plan's Stage C invited. Rationale: one
  `export A B C` node must yield one fact per name, which an `Option` cannot
  express, and an empty vector covers the name-less `export` without a second
  concept. The call site in `collect_items` becomes `observations.extend(...)`,
  which reads better than a conditional push. Date/Author: 2026-08-13,
  implementer.

- Decision: directive names are read by anchoring on the name the parser
  itself reports, not by skipping identifier tokens whose text matches a
  directive keyword. Rationale: the first implementation followed the plan's
  Stage C wording and filtered out any identifier whose text was `export`,
  `unexport`, `override` or `define`. Review found this silently discarded
  `export unexport`, which upstream parses cleanly as exporting a variable
  legitimately named `unexport`, while the report still claimed `complete` — a
  direct breach of the honesty constraint. Upstream's
  `VariableDefinition::name()` is the authority for the first name and returns
  `None` precisely when the line names nothing it could parse, so the
  identifier tokens preceding that name are exactly the prefix keywords
  consumed. This is also correct for `export export FOO`, where upstream
  consumes both leading `export` tokens. The plan's Stage C wording is
  superseded on this point. Date/Author: 2026-08-13, implementer, after stage
  review.

- Decision: an `export` line whose name upstream cannot determine yields no
  facts regardless of whether it is also a `define`. Rationale: the first
  implementation gated this on `is_directive_only`, which excludes `define`, so
  `export define FOO` and `export define` still aborted with exit code 2 —
  failing the plan's own acceptance criterion 3. Upstream models both with no
  name at all and diagnoses both, so dropping the facts leaves the report
  honestly `recovered`. A name-less definition that is not an export still
  fails loudly, so genuinely broken trees are not masked. Date/Author:
  2026-08-13, implementer, after stage review.

- Decision: an export line the parser cannot name yields a diagnostic of
  `makeutil`'s own rather than relying on upstream to have emitted one.
  Rationale: the second stage review showed upstream drops
  `override export override` without any error, so trusting it produced a
  `complete` report with the line missing. Emitting the diagnostic in the
  adapter makes the honesty guarantee independent of upstream. The cost is a
  second diagnostic on inputs upstream does diagnose, such as a bare `export`,
  which is noise rather than inaccuracy: both diagnostics are true and the
  status is `recovered` either way. Date/Author: 2026-08-13, implementer, after
  second stage review.

- Decision (review finding retained rather than fixed): the
  `|| context.is_export` arm of `assignment_operator` stays, although review
  showed it is unreachable by construction — reaching it with an absent
  operator requires `is_define`, which the first disjunct already covers.
  Rationale: the plan's `Interfaces and dependencies` section specifies this
  arm, and it keeps the function correct in isolation rather than only correct
  given its single caller. The doc comment previously claimed it was a live
  guard against upstream change, which was untrue; it now states plainly that
  it is unreachable as the caller is written. Date/Author: 2026-08-13,
  implementer, after second stage review.

- Decision (review nit declined): the extracted module keeps the name
  `makefile_export.rs` rather than being renamed to `makefile_variable.rs`.
  Rationale: review observed correctly that the module also owns the generic
  operator mapping for all eight operators, which is not export-specific. The
  name is however the one the plan's Stage C names as the extraction target,
  and the module comment states the wider scope. Renaming would diverge from
  the plan for a cosmetic gain. Date/Author: 2026-08-13, implementer, after
  stage review.

- TOLERANCE BREACH (recorded, not worked around): the scope tolerance for
  Stages B to D — "more than six files or more than 250 net lines" — is
  exceeded. The delivered change touches twelve code files and four
  documentation files, with 417 insertions against 69 deletions, so 348 net
  lines. Analysis: the tolerance contradicts the plan's own Stage B and Stage D
  instructions, which by themselves enumerate three new fixtures, a new
  integration test file holding five named tests, an extended case list in
  `tests/report_schema.rs`, a new Gherkin scenario in
  `tests/features/parse.feature`, its step and registration in
  `tests/parse_bdd.rs`, the adapter change, the adapter unit tests, and updates
  to `docs/users-guide.md` and `docs/design.md` — already more than six files
  before a single line is written. The overage therefore reflects a tolerance
  set too tightly, not scope creep: no file was touched that the plan did not
  name, except the module extraction the plan explicitly permits and its test
  file. The breach is recorded here and reported rather than engineered around,
  because compressing the work into six files would mean discarding
  deliverables the plan requires. Date/Author: 2026-08-13, implementer.

- BLOCKED: Stage E cannot be executed under the delegated authority for this
  work, which forbids pushes, pull requests, or issues against any repository
  other than the makeutil repository itself. Stage E's substance is a parser
  change in the separate `makefile-lossless` fork, followed by a revision bump
  here that can only point at a commit published in that fork. Consequence: the
  multi-name form `export A B C` continues to report `recovered` and to capture
  only the first name, exactly as the plan's Stage C go/no-go anticipates.
  Stages B to D, F, and G stand on their own: no form of `export` or `unexport`
  aborts any more, and the single-name case is `complete`.
  `multi_name_export_never_aborts` and `bare_export_names_are_all_collected`
  remain in their Stage C form, so whoever resumes Stage E has the pins already
  written and need only flip them. Date/Author: 2026-08-13, implementer.

- Decision: Stages B and C are delivered as a single commit rather than one
  commit each. Rationale: `AGENTS.md` forbids committing anything that fails a
  quality gate, and a Stage B commit is red by construction. Test-first order
  is preserved in the work itself and the red evidence is recorded in
  `Artefacts and notes`, so the discipline is auditable without committing a
  broken tree. Date/Author: 2026-08-13, implementer.

## Outcomes & retrospective

Recorded at the end of Stage G, with Stage E blocked.

Does `export A B C` parse to `complete` with all three names? No. It parses
without aborting and reports `recovered` with the first name only. The pinned
parser traps the second name in an error node and pushes the third out of the
variable node entirely, so `makeutil` cannot recover them. Closing this needs
Stage E, which is blocked on authority to change a separate repository. The
tests that would prove it are already written and merely need their
expectations flipped.

Did any snapshot change unexpectedly? No snapshot changed at all. The upstream
revision was never bumped, and both `insta` snapshots in `tests/snapshots/` are
byte-identical to their pre-change state, as is
`schemas/makeutil.parse.v1.schema.json`.

Did the `variables`-reuse representation cause confusion in review? Not the
representation itself, which review accepted twice. What review caught were
three defects in how the directive was recognized, all recorded in the
`Decision Log`. Identifying directive keywords by token text silently discarded
`export unexport`. Gating the no-facts path on "export and not define" left
`export define FOO` aborting with exit code 2. Trusting upstream to diagnose
every line it could not name produced a `complete` report with
`override export override` missing entirely.

Each breached the plan's own honesty or never-abort constraint while every gate
passed, and each was found by an adversarial reading of the *implementation*
rather than of the plan: the fixtures were drawn from the forms the plan
enumerated, so they could not catch inputs the plan had not imagined. The
recurring root cause is worth carrying forward — twice the implementation
leaned on an assumption about upstream behaviour, and both times the assumption
was false for some input. A guarantee the adapter enforces itself is worth more
than one inherited from a dependency.

The corrected implementation anchors on the name upstream reports rather than
on a keyword list, and emits its own diagnostic whenever an export line yields
no facts. Fourteen export and unexport forms are pinned by a parameterized test
asserting status and variable names together, so a form that stopped producing
its fact could not keep passing.

Two further defects were observed and deliberately left alone, both outside
this plan's scope and both deserving their own plan, ideally a shared one:
`foo: export BAR := baz` reports `complete` with prerequisites
`["export", "BAR", ":=", "baz"]`, and `override define FOO ... endef` still
aborts with exit code 2, losing the whole file.

## Context and orientation

Read this section in full before touching anything. It assumes no prior
knowledge of the repository.

### What the tool does

`makeutil` is a Rust command-line tool. Its only subcommand today is
`makeutil parse <path>`, which reads a single GNU Makefile and writes one JSON
document to standard output describing what it found: rules, variable
definitions, include directives, and any parser diagnostics. It never executes
`make` and never follows `include` directives; it only reports syntax.

The design is a hexagonal one — the domain owns the report types and defines a
port, and an adapter wraps the third-party parser so that no upstream type
leaks into the report. The layers are:

- `src/domain/mod.rs` — the report types that are serialized to JSON:
  `ParseReport`, `RuleFact`, `VariableFact`, `IncludeFact`, `ParseDiagnostic`,
  `ParseStatus`, and the `AssignmentOperator` enum. These derive `Serialize`
  and their field names are the JSON key names. `SCHEMA_VERSION` is `1`.
- `src/domain/location.rs` — `SourceSpan` (byte offsets) and `LocationIndex`,
  which converts byte offsets into one-based line and column positions.
- `src/ports.rs` — the `MakefileParser` trait, the `SyntaxObservation` enum that
  the adapter produces, and `ParserPortError`. `SyntaxObservation` is the
  internal, pre-location vocabulary; it has variants `Rule`, `Variable`,
  `Include`, and `Diagnostic`.
- `src/adapters/makefile.rs` — the adapter over the third-party parser crate. It
  walks the concrete syntax tree and produces `SyntaxObservation` values.
- `src/application.rs` — `parse_source`, which hashes the input, drives the
  parser, resolves every span into a location, and assembles a `ParseReport`.
  The `ReportAssembly::status` method (near the end of the file) is what decides
  `complete` versus `recovered`: any diagnostic at all means `recovered`.
- `src/adapters/cli.rs` — argument handling and the process exit policy.

"Concrete syntax tree" (CST) here means a lossless tree that retains every byte
of the input, including whitespace and comments, so the original text can be
reproduced exactly. The crate providing it is `makefile-lossless`, built on the
`rowan` library. `rowan` trees are made of *nodes* (which have a `SyntaxKind`
such as `VARIABLE`, `RULE`, `EXPR`, `ERROR`) and *tokens* (leaves such as
`IDENTIFIER`, `WHITESPACE`, `OPERATOR`, `NEWLINE`).

`Cargo.toml` pins `makefile-lossless = "=0.3.40"` and then redirects it with a
`[patch.crates-io]` entry to a specific git revision of a fork,
`8dd35801b75b332c2ac2f995ae398ef8238559fa`. This matters: the established way
to land an upstream parser change in this project is to make the change on that
fork and bump the revision in `[patch.crates-io]`. The published version number
`0.3.40` is unchanged by such a bump, so neither `ToolIdentity::default()` in
`src/domain/mod.rs` nor the `parser_version` constant in the JSON schema needs
to change.

### Where the defect lives

The abort originates in `assignment_operator` in `src/adapters/makefile.rs`,
lines 196-216. It maps the upstream operator string to the domain enum:

```rust
fn assignment_operator(
    operator: Option<&str>,
    is_define: bool,
) -> Result<AssignmentOperator, ParserPortError> {
    match operator {
        None if is_define => Ok(AssignmentOperator::Define),
        Some("=") => Ok(AssignmentOperator::Recursive),
        // ... other operators ...
        None => Err(ParserPortError::MissingField {
            field: "variable-assignment-operator",
        }),
    }
}
```

The final arm is the crash. A bare `export FOO` has no operator token and is
not a `define` block, so `operator` is `None` and `is_define` is `false`, and
the adapter returns `MissingField`. That error propagates out of
`variable_observation` (lines 174-194), out of `collect_items`, out of
`MakefileLosslessParser::parse`, through `ParseApplicationError::Parser`, and
into `src/adapters/cli.rs` line 197, which prints
`makeutil: parse-internal: <message>` and exits 2.

Note the ordering in `MakefileLosslessParser::parse` (lines 34-43):
`collect_items` runs *before* `collect_diagnostics`. That is why a file whose
only problem is one export line yields no diagnostics and no partial report —
the traversal dies before diagnostics are ever gathered.

There is a second, distinct abort on the same path. A bare `export` alone on a
line makes `VariableDefinition::name()` return `None`, so
`variable_observation` (line 180) returns
`MissingField { field: "variable-name" }` and the tool exits 2 with
`makeutil: parse-internal: required variable-name accessor was absent`.

### Verified current behaviour of every export form

The following were measured against the current default-branch build. Reproduce
them yourself in Stage A. "Exit 2" means the fatal `parse-internal` path; exit
1 means a `recovered` report was still printed; exit 0 means `complete`.

- `export FOO` — exit 2,
  `required variable-assignment-operator accessor was absent`. Upstream tree is
  clean with no errors; this is purely a makeutil defect.
- `export FOO BAR BAZ` — exit 2, same message. Upstream additionally reports
  `expected assignment operator`, and has already lost `BAR` into an error node
  and `BAZ` out of the variable node altogether.
- `export` alone — exit 2, `required variable-name accessor was absent`.
  Upstream reports `expected variable name`.
- `unexport FOO` — exit 1. Report is produced but is wrong: a rule with targets
  `["unexport", "FOO"]`, status `recovered`, diagnostic `expected ':'`.
- `unexport FOO BAR` — exit 1, same shape with targets
  `["unexport", "FOO", "BAR"]`.
- `unexport` alone — exit 1, a rule with targets `["unexport"]`.
- `export FOO := bar` — exit 0, `complete`. Correctly reported as a variable
  with `operator: ":="` and `exported: true`. Already works; must not regress.
- `foo: export BAR := baz` — exit 0, `complete`, but wrong: a rule with
  prerequisites `["export", "BAR", ":=", "baz"]`. Out of scope; see
  `Surprises & Discoveries`.
- `FOO := bar` followed by `export FOO` and then ordinary rules — exit 2. This
  is the real-world shape and demonstrates the blast radius: the assignment and
  every rule in the file are lost because of one directive line.

### How the tests are organized

- `src/adapters/makefile_tests.rs` — unit tests for the adapter, included into
  `src/adapters/makefile.rs` at the bottom via
  `#[cfg(test)] #[path = "makefile_tests.rs"] mod tests;`. These call private
  helpers such as `assignment_operator` and `condition_kind` directly. Note the
  existing test `ordinary_variable_requires_an_operator`, which asserts the
  very behaviour this plan changes — it must be updated, not deleted.
- `tests/fixtures/makefiles/` — Makefile fixtures: `all-facts.mk` (the complete
  happy path), `recovered.mk`, `multiline-define.mk`, and
  `conditional-error-directive.mk`.
- `tests/corpus.rs` — the "unsupported syntax corpus". Its module comment states
  the policy directly: these tests pin honest degradation for constructs the
  tool cannot represent, and are designed to fail on purpose if an upstream
  release learns the construct, so the pin and the expectations are revisited
  together. Stage F adds to this file.
- `tests/report_schema.rs` — validates serialized reports against
  `schemas/makeutil.parse.v1.schema.json` using the `jsonschema` crate, checks
  that a deliberately malformed document is rejected, and holds two `insta`
  snapshot tests whose stored output lives in `tests/snapshots/`.
- `tests/parse_bdd.rs` and `tests/features/parse.feature` — behaviour-driven
  tests using `rstest-bdd`. The feature file holds `Scenario` blocks in Gherkin;
  `tests/parse_bdd.rs` holds the `#[given]`, `#[when]`, `#[then]` step
  functions and a `World` struct carrying arguments, stdin, stdout, stderr, and
  exit code.
- `tests/cli_e2e.rs` — end-to-end tests that run the built binary.
- `tests/domain_contract.rs`, `tests/output_failures.rs`,
  `tests/source_adapter.rs` — not affected by this plan.

### Commit gates

Run these from the repository root before every commit. Run them sequentially,
never in parallel — the build cache is shared and parallel invocations fight
over it.

```sh
make check-fmt
make lint
make typecheck
make test
make markdownlint
make provenance
```

`make fmt` applies formatting fixes (`cargo +nightly fmt --all` plus Markdown
formatting) and must be run after editing any Markdown. `make test` prefers
`cargo nextest run` when available and falls back to `cargo test`, and also
runs doctests. `make markdownlint` additionally runs the `spelling` and
`provenance` targets. Prose must use en-GB Oxford spelling (`organize`,
`standardize`, `behaviour`, `colour`) and Markdown paragraphs must wrap at 80
columns.

## Plan of work

### Stage A: reproduce and orient (no code changes)

Confirm the defect on your own working tree before changing anything, so you
know the baseline is what this plan describes. Build the binary and run it over
each form listed under "Verified current behaviour of every export form" above,
checking the exit code and message of each. Record any divergence from the
table in `Surprises & Discoveries` before proceeding — a divergence means the
upstream pin has moved and the rest of this plan needs re-checking.

Go/no-go: proceed only if `export FOO` exits 2 with the
`variable-assignment-operator` message.

### Stage B: red tests and fixtures

Add the failing tests first. Every test added here must fail before Stage C,
and each must fail for the reason stated, not for an unrelated reason such as a
missing fixture file.

Create two fixtures.

`tests/fixtures/makefiles/bare-export.mk` — the single-name and already-working
forms, which Stage C alone must make `complete`:

```makefile
MOLD_VERSION_FILE := .mold-version
RUST_TOOLCHAIN_FILE := rust-toolchain.toml

export MOLD_VERSION_FILE
export RUST_TOOLCHAIN_FILE
export CARGO_TERM_COLOR := always

build:
	@echo building
```

`tests/fixtures/makefiles/export-directive-list.mk` — the real-world multi-name
shape, plus the name-less and `unexport` forms. This fixture stays `recovered`
after Stage C and becomes partially better after Stage E:

```makefile
MOLD_VERSION_FILE := .mold-version
MOLD_SHA256SUMS_FILE := .mold-sha256sums
RUST_TOOLCHAIN_FILE := rust-toolchain.toml

export MOLD_VERSION_FILE MOLD_SHA256SUMS_FILE RUST_TOOLCHAIN_FILE

check:
	@echo checking
```

Create a third fixture for the forms that remain unrepresentable,
`tests/fixtures/makefiles/export-directive-limits.mk`:

```makefile
FOO := 1
export
unexport FOO
```

Then add these tests.

In `src/adapters/makefile_tests.rs`, replace the existing test
`ordinary_variable_requires_an_operator` with a test asserting the new
behaviour, and add a companion for the directive form. The name-less signature
of `assignment_operator` will change in Stage C; write the test against the
intended new signature so that it fails to compile first, which is a valid red
state here. Add:

- `bare_export_directive_uses_empty_schema_variant`, asserting that an
  operator-less, non-`define`, exported variable maps to
  `AssignmentOperator::Define` (the empty-string schema variant) rather than
  producing `ParserPortError::MissingField`.
- `plain_variable_without_operator_is_still_rejected`, asserting that an
  operator-less variable with neither `define` nor `export` still yields
  `MissingField { field: "variable-assignment-operator" }`. This guards against
  over-relaxing the check.
- `bare_export_names_are_all_collected`, calling the new helper described in
  Stage C over a parsed `export FOO BAR BAZ` tree. Before Stage E this asserts
  the single name `["FOO"]`; Stage E flips it to `["FOO", "BAR", "BAZ"]`.

In `tests/report_schema.rs`, extend the `reports_validate_against_schema` case
list with the three new fixtures, so the schema contract is checked for each.
These will fail at Stage B because `parse_source` returns an error for the
first two fixtures rather than a report.

Add a new integration test file, `tests/export_directives.rs`, with a module
comment explaining that it pins the report shape for GNU Make's export family.
It must contain:

- `single_name_bare_export_is_complete` — parses `bare-export.mk` and asserts
  `status == ParseStatus::Complete`, `diagnostics.is_empty()`, and that
  `variables` contains entries named `MOLD_VERSION_FILE` and
  `RUST_TOOLCHAIN_FILE` with `operator == AssignmentOperator::Define`,
  `exported == true`, `define_block == false`, and `raw_value.is_empty()`. This
  is the test that pins the consumer-facing discriminating predicate; say so in
  its doc comment.
- `assignments_and_directives_are_distinguishable` — on the same report, asserts
  that the entry for the assignment `CARGO_TERM_COLOR` has
  `operator == AssignmentOperator::Simple`, proving directives and assignments
  are told apart by the operator alone.
- `facts_after_a_bare_export_survive` — asserts the `build` rule is present,
  proving the blast radius is gone.
- `multi_name_export_never_aborts` — parses `export-directive-list.mk` and
  asserts only that `parse_source` returns `Ok`. Before Stage E it additionally
  asserts `status == ParseStatus::Recovered`; Stage E changes this to
  `Complete` with all three names present.
- `name_less_export_degrades_to_a_diagnostic` — parses
  `export-directive-limits.mk`, asserts `Ok`, asserts
  `status == ParseStatus::Recovered`, asserts at least one diagnostic, and
  asserts no variable fact was invented for the name-less `export`.

Add one behavioural scenario. In `tests/features/parse.feature`, append:

```gherkin
  Scenario: Parse a Makefile that exports already-defined variables
    Given a Makefile fixture that exports already-defined variables
    When makeutil parses the fixture by path
    Then stdout contains one schema version 1 JSON document
    And the process exits with code 0
    And stderr is empty
```

In `tests/parse_bdd.rs`, add the matching `#[given]` step, following the shape
of the existing `complete_fixture` step, pointing at
`tests/fixtures/makefiles/bare-export.mk`, and register the scenario with the
`#[scenario]` attribute in the same style as the existing scenarios.

Red validation for this stage is described under `Concrete steps`.

Go/no-go: proceed only when every new test fails and the failures are the
expected ones. If any new test passes at this point, the fixture is wrong.

### Stage C: minimal implementation

Make the smallest change that turns every red test green except the ones
explicitly deferred to Stage E.

Change `assignment_operator` in `src/adapters/makefile.rs` (lines 196-216) so
that an absent operator is acceptable for a bare export directive as well as
for a `define` block. The function currently takes `(Option<&str>, bool)`; two
booleans in a row would be an unreadable signature, so introduce a small
parameter struct in the same file rather than adding a positional `bool`:

```rust
/// Modifiers that make an absent assignment operator legitimate.
#[derive(Debug, Clone, Copy)]
struct OperatorContext {
    /// Whether the definition is a `define` ... `endef` block.
    is_define: bool,
    /// Whether the `export` directive keyword is present.
    is_export: bool,
}
```

and change the two `None` arms to:

```rust
None if context.is_define || context.is_export => Ok(AssignmentOperator::Define),
None => Err(ParserPortError::MissingField {
    field: "variable-assignment-operator",
}),
```

Update the single call site in `variable_observation` (line 183) to pass an
`OperatorContext` built from `variable.is_define()` and `variable.is_export()`.

Then handle the name-less `export`. In `variable_observation` (lines 174-194),
`variable.name()` currently produces `MissingField` when absent. A bare
`export` with no names is a real GNU Make construct meaning "export every
variable", which schema version 1 cannot represent, and upstream already emits
an `expected variable name` diagnostic for it. So the correct behaviour is to
produce no fact at all and let the upstream diagnostic drive
`status: "recovered"`. Change `variable_observation` to return
`Result<Option<SyntaxObservation>, ParserPortError>`, returning `Ok(None)` when
`variable.is_export()` is true and `variable.name()` is `None`, and update the
`TraversalEvent::Item(MakefileItem::Variable(...))` arm of `collect_items`
(lines 72-74) to push only when a fact was produced. Keep the `MissingField`
error for a name-less variable that is *not* an export, so genuinely broken
trees still fail loudly.

Add the name-collection helper that Stage E will rely on. In
`src/adapters/makefile.rs`, add:

```rust
/// Collect every identifier a directive-only `export` line names.
///
/// A bare `export A B C` names three variables. Upstream's
/// `VariableDefinition::name()` returns only the first, so walk the node's own
/// identifier tokens, skipping the directive keywords themselves.
fn directive_names(variable: &VariableDefinition) -> Vec<String>;
```

It walks `variable.syntax().children_with_tokens()`, keeps tokens whose kind is
`SyntaxKind::IDENTIFIER` and whose text is not `export`, `unexport`,
`override`, or `define`, and returns their texts in source order. Before Stage
E this returns one name for the multi-name input, because upstream has already
moved the rest out of the node; that is expected and is what
`bare_export_names_are_all_collected` asserts at this stage.

Use `directive_names` in `variable_observation` when the operator is absent and
`is_export` is true: emit one `SyntaxObservation::Variable` per name, each with
`raw_value` empty, `exported: true`, `define_block: false`, and the span of the
whole directive node. Emitting several observations from one node means
`variable_observation` should return a `Vec<SyntaxObservation>` rather than an
`Option`; prefer that shape over the `Option` above if it reads more cleanly,
and record the choice in `Decision Log`. An empty vector then naturally covers
the name-less case.

If `src/adapters/makefile.rs` approaches the 400-line limit, extract the export
handling into `src/adapters/makefile_export.rs` and declare it from
`src/adapters/mod.rs`, keeping the module comment convention.

Go/no-go: every Stage B test passes except `multi_name_export_never_aborts`'s
eventual `Complete` assertion and `bare_export_names_are_all_collected`'s
eventual three-name assertion, both of which remain in their Stage C form.

### Stage D: refactor, documentation, and gates

Clean up without changing behaviour, then document.

Update `docs/users-guide.md`, in the "Interpret results" section, with a short
prose paragraph explaining that a bare `export NAME` directive appears in the
`variables` array with an empty `operator` and an empty `raw_value`, that
`exported` is `true` and `define_block` is `false` for such an entry, and that
a consumer wanting only genuine assignments should filter on a non-empty
`operator`. Give the exact predicate. This paragraph is the mitigation for the
conflation risk; do not skip it.

Update `docs/design.md` to record the representation decision and reference
this plan. If the decision is judged substantive enough to warrant its own
Architectural Decision Record, add one under `docs/adrs/` following the
numbering and style of `docs/adrs/0001-single-file-gnu-make-parse.md` and
reference it from the design document, as `AGENTS.md` requires.

Refresh the `insta` snapshots if and only if a snapshot legitimately changed.
Do not accept a snapshot change you cannot explain — an unexplained diff in
`tests/snapshots/report_schema__all_fact_variants_have_stable_json.snap` means
Stage C altered behaviour for inputs it should not have touched.

Run every commit gate listed under `Commit gates`. Commit.

### Stage E: upstream parser fix for multi-name exports

This stage changes the pinned parser fork, not this repository's `src/`.

Clone the fork at the currently pinned revision
`8dd35801b75b332c2ac2f995ae398ef8238559fa` — the URL is in the
`[patch.crates-io]` section of `Cargo.toml`; take it from there rather than
transcribing it into any document. The relevant function is `parse_assignment`
in the upstream file `src/lossless.rs`, roughly lines 733 to 816. Its current
structure is: skip whitespace; consume up to two directive keywords (`export`,
`override`) if present; consume the variable name; skip whitespace; then match
on the next token. That final match has an arm for a valid operator, an arm for
`NEWLINE` (already commented "Bare `export VARNAME` without assignment operator
is valid GNU Make"), an arm for end of input, and a catch-all
`_ => self.error("expected assignment operator".to_string())`.

The catch-all is what breaks `export A B C`: after `FOO`, the next token is the
identifier `BAR`, which falls into the catch-all.

The change: record whether a directive keyword was consumed in the prefix loop,
and in the catch-all arm, when a directive keyword *was* consumed and the next
token is an `IDENTIFIER`, consume alternating whitespace and identifier tokens
into the same `VARIABLE` node until `NEWLINE` or end of input, without emitting
an error. When no directive keyword was consumed, keep the existing error — a
plain `FOO BAR` line is still malformed.

Write upstream tests first, in the upstream repository, following its existing
test style in `src/lossless.rs` (see `test_parse_export_assign` for the pattern
of asserting a rendered tree). At minimum: `export FOO BAR BAZ\n` produces one
`VARIABLE` node spanning the whole line with three identifier tokens after the
`export` keyword, no `ERROR` node, and no reported errors; and `FOO BAR\n`
without a directive still errors as before.

Then, in this repository:

1. Bump the `rev` in `[patch.crates-io]` in `Cargo.toml` to the new upstream
   commit and run `cargo update -p makefile-lossless` so `Cargo.lock` follows.
2. Flip `bare_export_names_are_all_collected` in
   `src/adapters/makefile_tests.rs` to expect `["FOO", "BAR", "BAZ"]`.
3. Flip `multi_name_export_never_aborts` in `tests/export_directives.rs` to
   assert `status == ParseStatus::Complete`, no diagnostics, and three variable
   facts named `MOLD_VERSION_FILE`, `MOLD_SHA256SUMS_FILE`, and
   `RUST_TOOLCHAIN_FILE`, each with an empty operator and `exported == true`.
   Rename the test to `multi_name_export_is_complete` at this point.
4. Re-run the full gate set and inspect both snapshots for unexpected diffs.

Do not change `parser_version` anywhere: the published version string remains
`0.3.40`, so `ToolIdentity::default()` in `src/domain/mod.rs` and the
`parser_version` const in `schemas/makeutil.parse.v1.schema.json` are untouched.

Go/no-go: if the upstream change causes any snapshot diff outside the export
fixtures, stop and escalate rather than accepting the snapshot.

### Stage F: pin the `unexport` limitation

Add a test to `tests/corpus.rs`, whose module comment already describes exactly
this policy, named `unexport_directive_degrades_honestly`. It parses
`tests/fixtures/makefiles/export-directive-limits.mk` and asserts that the
parse succeeds, that `status` is `Recovered`, that at least one diagnostic is
present, and that the misleading rule fact whose first target is `unexport` is
present — pinning the current behaviour explicitly so that if a future upstream
release learns `unexport`, this test fails and forces the expectations to be
revisited alongside the pin, exactly as the file's existing tests do.

Document the limitation in `docs/users-guide.md` in one short paragraph: today
an `unexport` directive is reported as a rule and forces `recovered`, and
faithful support awaits a schema version able to express an explicit un-export.
Do not attempt to fix it in this plan.

### Stage G: consumer re-pin note

Downstream repositories consume `makeutil` by pinning a specific commit SHA, so
the fix reaches them only when they move their pin. Add a short entry to the
repository's release or changelog documentation (follow whatever convention
`docs/contents.md` indexes; if none exists, add the note to `docs/design.md`
under the export representation section) recording: the merge commit SHA that
carries this fix, the fact that bare `export` directives now appear in
`variables` with an empty `operator`, and the fact that `unexport` remains
unsupported. Changes inside consumer repositories are explicitly out of scope
for this plan.

## Concrete steps

All commands run from the repository root.

Stage A, reproduce:

```console
$ cargo build --bin makeutil
$ printf 'export FOO\n' > /tmp/mk-a.mk
$ ./target/debug/makeutil parse /tmp/mk-a.mk; echo "exit=$?"
makeutil: parse-internal: required variable-assignment-operator accessor was absent
exit=2
$ printf 'export\n' > /tmp/mk-b.mk
$ ./target/debug/makeutil parse /tmp/mk-b.mk; echo "exit=$?"
makeutil: parse-internal: required variable-name accessor was absent
exit=2
$ printf 'unexport FOO BAR\n' > /tmp/mk-c.mk
$ ./target/debug/makeutil parse /tmp/mk-c.mk > /dev/null; echo "exit=$?"
exit=1
```

Stage B, red. Run the new tests and confirm they fail:

```sh
cargo test --test export_directives
```

Expect failures of the form
`Parser(MissingField { field: "variable-assignment-operator" })` reported
through `parse_source` returning `Err`, not assertion failures about values.
Also run:

```sh
cargo test --lib adapters::makefile::tests
cargo test --test report_schema
cargo test --test parse_bdd
```

The adapter unit tests may fail to compile at this point because
`assignment_operator` does not yet take an `OperatorContext`. A compile failure
naming that function is an acceptable red state; a compile failure naming
anything else is not — fix the test instead.

Stage C, green:

```sh
cargo test --test export_directives
cargo test --lib
cargo test --test report_schema
cargo test --test parse_bdd
./target/debug/makeutil parse tests/fixtures/makefiles/bare-export.mk; echo "exit=$?"
```

The last command must print a JSON document containing `"status":"complete"`
and exit 0.

Stage D, gates, run sequentially:

```sh
make fmt
make check-fmt
make lint
make typecheck
make test
make markdownlint
make provenance
```

Stage E, after the upstream revision bump:

```sh
cargo update -p makefile-lossless
cargo test --test export_directives
cargo build --bin makeutil
./target/debug/makeutil parse tests/fixtures/makefiles/export-directive-list.mk; echo "exit=$?"
```

Expect `"status":"complete"`, exit 0, and three variable entries with
`"operator":""` and `"exported":true`. Then rerun the full gate set.

Stage F:

```sh
cargo test --test corpus
```

## Validation and acceptance

Acceptance is behavioural, not structural.

1. A Makefile that assigns variables and then exports one of them by name parses
   to `"status": "complete"` with exit code 0, and every rule and assignment in
   the file is present in the report. Verified by
   `single_name_bare_export_is_complete` and
   `facts_after_a_bare_export_survive` in `tests/export_directives.rs`, and by
   the new BDD scenario "Parse a Makefile that exports already-defined
   variables".
2. A Makefile containing `export A B C` on one line parses to
   `"status": "complete"` with exit code 0 and all three names present in
   `variables`. Verified by `multi_name_export_is_complete` after Stage E.
3. No form of `export` or `unexport` produces a `parse-internal` message or exit
   code 2. Verified by `multi_name_export_never_aborts`,
   `name_less_export_degrades_to_a_diagnostic`, and
   `unexport_directive_degrades_honestly`.
4. Bare export directives are distinguishable from assignments by the
   `operator` field alone. Verified by
   `assignments_and_directives_are_distinguishable`.
5. Every new fixture's report validates against the unchanged version-1 schema.
   Verified by the extended `reports_validate_against_schema` case list in
   `tests/report_schema.rs`.
6. Nothing that worked before regresses: `export FOO := bar` still reports
   `operator: ":="` with `exported: true`, and a plain operator-less variable
   still fails loudly. Verified by
   `plain_variable_without_operator_is_still_rejected` and the unchanged
   existing suites.

Red-Green-Refactor evidence to record in `Artefacts and notes` as the work
proceeds:

- Red: `cargo test --test export_directives` before Stage C, showing failures
  caused by `parse_source` returning `Err(Parser(MissingField { .. }))`.
- Green: the same command after Stage C, showing all tests passing except the
  two explicitly deferred to Stage E.
- Refactor: `make check-fmt && make lint && make typecheck && make test`, all
  passing, after the Stage D cleanup.

Quality criteria for "done":

- Tests: `make test` passes with no ignored or skipped new tests, and both
  `insta` snapshots are either unchanged or changed for an explained reason.
- Lint and typecheck: `make lint` and `make typecheck` pass with no warnings and
  no new lint suppressions anywhere in the diff.
- Documentation: `make markdownlint` passes, which also runs the spelling and
  provenance gates.
- Schema: `schemas/makeutil.parse.v1.schema.json` is byte-identical to its state
  before this work.

## Idempotence and recovery

Every step is re-runnable. The test commands and the `make` gates are pure
checks. `make fmt` is idempotent. `cargo update -p makefile-lossless` is
idempotent once the `rev` is set.

The only step with a wider blast radius is the Stage E revision bump. To roll
it back, restore the previous `rev` in `[patch.crates-io]` in `Cargo.toml`, run
`cargo update -p makefile-lossless`, and revert the Stage E test flips. Because
Stage E is a separate commit from Stages B to D, `git revert` of that single
commit restores the Stage D state, in which bare single-name exports already
work.

Do not accept an `insta` snapshot with `cargo insta accept` without first
reading the diff. If a snapshot was accepted in error, `git checkout` the file
under `tests/snapshots/` and rerun.

Temporary Makefiles written under `/tmp` during Stage A can be deleted freely;
nothing in the repository depends on them.

## Artefacts and notes

### Red-Green-Refactor evidence

Stage A reproduction, run against the pre-change build: every row of the
behaviour table above reproduced exactly. `export FOO`, `export FOO BAR BAZ` and
`export` all exited 2; the three `unexport` forms exited 1; both
`export FOO := bar` and `foo: export BAR := baz` exited 0.

Stage B red, `cargo test --test export_directives`:

```plaintext
running 5 tests
test facts_after_a_bare_export_survive ... FAILED
test assignments_and_directives_are_distinguishable ... FAILED
test name_less_export_degrades_to_a_diagnostic ... FAILED
test multi_name_export_never_aborts ... FAILED
test single_name_bare_export_is_complete ... FAILED

---- name_less_export_degrades_to_a_diagnostic stdout ----
Error: Parser(MissingField { field: "variable-name" })
---- single_name_bare_export_is_complete stdout ----
Error: Parser(MissingField { field: "variable-assignment-operator" })
```

The other three failed with the same `variable-assignment-operator` error, so
every failure was the abort itself rather than an assertion about values.
`cargo test --test report_schema` failed the three new fixture cases with the
same two errors, `cargo test --lib` failed to compile with
`unresolved imports super::OperatorContext, super::directive_names` as
anticipated, and `cargo test --test parse_bdd` failed only the new scenario,
with exit code 2 where 0 was expected.

Stage C green, `cargo test --test export_directives`: 5 passed, 0 failed.
`cargo test --lib`: 20 passed, 0 failed.

Stage review red, after the reviewer identified the keyword-text defect. Both
were reproduced against the built binary before being fixed:

```console
$ printf 'A := 1\nexport unexport\nb:\n\techo\n' > t.mk
$ makeutil parse t.mk    # status=complete, variables=['A'], exit=0
$ printf 'export define FOO\nbody\nendef\n' > t.mk
$ makeutil parse t.mk
makeutil: parse-internal: required variable-name accessor was absent
exit=2
```

Stage review green: `export unexport` now reports `complete` with the
`unexport` fact present, and both `export define` forms report `recovered` with
exit code 1 and no invented facts.

Final gate run, all sequential and all passing: `make check-fmt`, `make lint`
(including the `whitaker` driver), `make typecheck`, `make test` (129 tests
run, 129 passed, 0 skipped, plus 3 doctests), and `make markdownlint` (which
also runs `spelling` and `provenance`), 0 errors.

Both `insta` snapshots and `schemas/makeutil.parse.v1.schema.json` are
unchanged from their pre-change state.

### Captured concrete syntax trees

The concrete syntax trees below were captured against the pinned parser
revision and justify the staging. Keep them; they are the evidence that the
single-name case is a makeutil defect and the multi-name case is an upstream
one.

Single-name bare export — clean tree, no errors, purely a makeutil defect:

```plaintext
ROOT@0..11
  VARIABLE@0..11
    IDENTIFIER@0..6 "export"
    WHITESPACE@6..7 " "
    IDENTIFIER@7..10 "FOO"
    NEWLINE@10..11 "\n"
```

Multi-name export — the second name is trapped in an error node and the third
escapes the variable node entirely, so makeutil cannot recover them without an
upstream change:

```plaintext
ROOT@0..19
  VARIABLE@0..14
    IDENTIFIER@0..6 "export"
    WHITESPACE@6..7 " "
    IDENTIFIER@7..10 "FOO"
    WHITESPACE@10..11 " "
    ERROR@11..14
      IDENTIFIER@11..14 "BAR"
  WHITESPACE@14..15 " "
  IDENTIFIER@15..18 "BAZ"
  NEWLINE@18..19 "\n"
```

Reported error for the above: `expected assignment operator` at range `10..11`.

Name-less export — upstream already diagnoses it, so makeutil only needs to
stop aborting:

```plaintext
ROOT@0..7
  VARIABLE@0..7
    IDENTIFIER@0..6 "export"
    ERROR@6..7
      NEWLINE@6..7 "\n"
```

`unexport` — parsed as a rule, which is why it never aborts and why it is
nonetheless wrong:

```plaintext
ROOT@0..17
  RULE@0..17
    TARGETS@0..16
      IDENTIFIER@0..8 "unexport"
      WHITESPACE@8..9 " "
      IDENTIFIER@9..12 "FOO"
      WHITESPACE@12..13 " "
      IDENTIFIER@13..16 "BAR"
    ERROR@16..17
      NEWLINE@16..17 "\n"
```

Expected JSON shape for a bare export fact after Stage C, abbreviated:

```json
{
  "ordinal": 2,
  "name": "MOLD_VERSION_FILE",
  "operator": "",
  "raw_value": "",
  "exported": true,
  "overridden": false,
  "define_block": false,
  "conditions": [],
  "location": { "start_byte": 0, "end_byte": 0, "start_line": 1, "start_column": 1, "end_line": 1, "end_column": 1 }
}
```

The `location` values above are placeholders; the real ones come from the
directive's span.

## Interfaces and dependencies

No new crate dependencies. The parser remains `makefile-lossless`, redirected by
`[patch.crates-io]` in `Cargo.toml` to a git revision; Stage E bumps that
revision and nothing else about the dependency graph.

At the end of Stage C, the following must exist in `src/adapters/makefile.rs`
(or in `src/adapters/makefile_export.rs` if the line budget forces an
extraction), all private to the crate:

```rust
/// Modifiers that make an absent assignment operator legitimate.
#[derive(Debug, Clone, Copy)]
struct OperatorContext {
    is_define: bool,
    is_export: bool,
}

fn assignment_operator(
    operator: Option<&str>,
    context: OperatorContext,
) -> Result<AssignmentOperator, ParserPortError>;

/// Collect every identifier a directive-only `export` line names.
fn directive_names(variable: &VariableDefinition) -> Vec<String>;
```

and `variable_observation` must return a collection of observations rather than
exactly one, so that a single `export A B C` node yields one
`SyntaxObservation::Variable` per exported name and a name-less `export` yields
none:

```rust
fn variable_observation(
    variable: &VariableDefinition,
    conditions: &[ConditionObservation],
    source_length: usize,
) -> Result<Vec<SyntaxObservation>, ParserPortError>;
```

Everything in `src/domain/mod.rs`, `src/ports.rs`, `src/application.rs`, and
`src/adapters/cli.rs` keeps its current public shape. `AssignmentOperator`
gains no new variant; the existing `Define` variant, which serializes to the
empty string, carries the bare-export case. If a new variant seems necessary,
that is a tolerance breach — stop and escalate, because it would change the
schema's `operator` enum.
