# ADR-0002: Represent bare `export` directives as valueless variable facts

## Status

Accepted on 2026-08-13

## Context

GNU Make's `export` keyword serves two roles. It can modify a definition, as in
`export FOO := bar`, or stand alone as a directive naming variables assigned
elsewhere, as in `export FOO BAR`. The directive form carries no assignment
operator and no value.

`makeutil` originally treated an absent assignment operator as a broken syntax
tree and returned a missing-field error, which surfaced as a fatal
`parse-internal` message and exit code 2. Because facts are collected before
diagnostics, one such line destroyed the report for the whole file: every rule,
variable, and include was lost. Downstream consumers therefore recorded an
operational error for an entire repository rather than a set of facts with one
gap. The directive form is common in real Makefiles, so this was a blocking
defect rather than an edge case.

Schema version 1 sets `"additionalProperties": false` at every level, and
consumers pin `makeutil` by commit SHA and validate reports against the
published schema. Any new key is therefore a breaking change requiring a schema
version 2 and a coordinated re-pin by every consumer.

Consumers also fail closed on any `parse.status` other than `complete`, so a
representation that forces `recovered` leaves them just as blocked as an abort
did.

## Decision

A bare `export NAME` directive is reported as an entry in the existing
`variables` array, using the operator enum's existing empty-string variant,
with an empty `raw_value`, `exported` true, and `define_block` false. A
directive naming several variables yields one entry per name.

The discriminating predicate for consumers is
`operator == "" && define_block == false`, which identifies an export directive
rather than an assignment. The empty operator is shared with `define` blocks,
which is why `define_block` is part of the predicate.

Neither `schema_version` nor `schemas/makeutil.parse.v1.schema.json` changes,
because the empty operator was already in the enum for `define` blocks and no
new key or enum member is introduced.

A form the parser cannot name — a bare `export` with no names, which means
"export every variable", or `export define NAME` — yields no entry rather than
an invented one. The parser already diagnoses these, so the report degrades to
`recovered` rather than falsely claiming `complete`.

The names on a directive line are read from the definition node's identifier
tokens, anchored on the name the parser itself reports rather than by skipping
identifiers whose text matches a directive keyword. A variable may legitimately
be called `export` or `unexport`, so keyword-text filtering would silently
discard real facts.

`unexport` remains unrepresented and is documented as a known gap.

## Alternatives considered

### Add a new top-level `exports` array

Rejected. This is the most honest representation, but `additionalProperties` is
`false` on the root object, so every consumer validating a report against the
version 1 schema would reject reports containing it. That is a breaking change
requiring a schema version 2 and a coordinated re-pin by every consumer —
disproportionate to a crash fix, and it would leave consumers blocked until
they moved.

### Record the directive only as a recoverable diagnostic

Rejected. It stops the abort but forces `status: "recovered"`, and consumers
fail closed on any status other than `complete`. Every Makefile containing a
bare export would remain effectively unparsable downstream, which fails the
purpose of the change.

### Add a new operator enum variant for the directive form

Rejected. The operator enum is a closed set in the version 1 schema, so a new
member is as breaking as a new key.

## Consequences

### Positive

- No form of `export` or `unexport` aborts the parse, so one directive line can
  no longer destroy the report for a whole file.
- A Makefile that assigns variables and then exports them by name reports
  `complete` with exit code 0, unblocking consumers immediately.
- No schema file changes and no consumer re-pin is required beyond moving to a
  commit that carries the fix.

### Negative

- Conflation: a consumer treating every entry in `variables` as an assignment
  now sees extra entries for export directives, and a name may appear twice —
  once for its assignment and once for the directive that exports it. This is
  mitigated by the documented predicate in the users' guide and pinned by a
  test.
- The representation cannot express "export every variable", so that form is
  reported as an absence plus a diagnostic rather than as a fact.

### Neutral

- A future schema version 2 could introduce a dedicated `exports` array and an
  explicit un-export representation. This decision does not preclude it; it
  defers it until a schema break is warranted on its own merits.

## Acceptance criteria

1. A Makefile that assigns variables and then exports one of them by name
   reports `complete` with exit code 0, and every rule and assignment in the
   file is present.
2. No form of `export` or `unexport` produces a `parse-internal` message or
   exit code 2.
3. Bare export directives are distinguishable from assignments by the
   `operator` field alone.
4. `schemas/makeutil.parse.v1.schema.json` is unchanged and every new fixture's
   report validates against it.
5. An operator-less definition that is neither a `define` nor an `export` still
   fails loudly, so genuinely broken trees are not masked.
