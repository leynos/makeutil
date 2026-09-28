# ADR-0003: Report bare expansion lines by what GNU Make can make of them

## Status

Accepted on 2026-09-28.

## Date

2026-09-28

## Context and Problem Statement

A top-level line may hold nothing but a function call or variable expansion,
such as `$(info ...)`, `$(error ...)` or `$(eval ...)`. The common case is a
read-time guard: `$(error ...)` inside an `ifeq` or `ifneq` block, rejecting a
bad setting before any recipe runs.

The parser had no item for such a line. It read the line as a rule missing its
colon, so each one added a spurious rule, forced `status: "recovered"`, and
emitted an `expected ':'` diagnostic. Consumers fail closed on any status other
than `complete`, so a well-formed Makefile with two guards was unusable
downstream. Its report also carried two rules that did not exist.

The diagnostics were misplaced as well. The parser computed the line-channel
line from the unconsumed token count, which always resolved to the end of
input. It read the positioned-channel range through an index that ran in the
opposite direction to the token stack, which named the mirror-image token.

GNU Make 4.4.1 expands such a line and parses the result:

- `$(info ...)`, `$(warning ...)` and `$(error ...)` expand to empty text. The
  line defines nothing; `$(error ...)` stops `make` when it is reached.
- `$(eval E := x)` defines the variable `E`.
- A bare `$(R)`, where `R := foo: ; @echo x`, defines the rule `foo`.
- A `$(foreach ...)` or `$(call ...)` can expand to either.

A static parse cannot expand, so it cannot know what the last three define.

## Decision

The parser fork keeps a bare expansion line as its own item,
`MakefileItem::Expansion`, exposing the references written on the line and
whether anything else is on it. A line counts as an expansion when it starts
with `$` and carries no operator outside a reference, across line
continuations. A colon or assignment operator outside the references keeps the
existing rule or assignment reading. The fork's diagnostics also now report the
line and range of the token they occur at.

`makeutil` reads each expansion line as follows:

- A line holding only calls to `info`, `warning` or `error`, with at most
  whitespace and a trailing comment beside them, adds no fact and no
  diagnostic. The report stays `complete`.
- Any other expansion line adds one diagnostic,
  `expansion line may define rules or variables a static parse cannot see`,
  located on that line. The report is `recovered`, so consumers still fail
  closed exactly where the report cannot see what the file defines.

No schema file changes. The diagnostic uses the existing diagnostic shape.

## Alternatives considered

### Add an `expansions` fact array

Rejected. It would record every expansion line, but `additionalProperties` is
`false` on the root object, so it is a schema version 2 and a coordinated
re-pin by every consumer. A consumer still could not learn what an `eval`
defines from the line's text, so it gains little over a diagnostic.

### Treat every bare expansion as fact-free

Rejected. `complete` would then be false for any Makefile that defines rules
through `$(eval ...)` or a bare `$(VAR)`. The report would silently omit rules
and variables that GNU Make creates.

### Keep reading the line as a rule

Rejected. It invents rules, and it keeps well-formed Makefiles with read-time
guards `recovered` for no reason a consumer can act on.

## Consequences

### Positive

- A Makefile whose only expansion lines are `info`, `warning` or `error` calls,
  including guarded `$(error ...)` lines, reports `complete` with every real
  rule and no invented one.
- A line that may define structure is named precisely, on its own line, rather
  than hidden behind a misplaced `expected ':'`.
- Every parser diagnostic, not only these, now reports the line it occurs on.

### Negative

- A Makefile that defines rules through `$(eval ...)` remains `recovered`.
  That is the intended fail-closed outcome, but such consumers stay blocked
  until they restructure or accept the gap.
- `$(if ...)` or `$(foreach ...)` wrapped around `$(error ...)` also expands to
  empty text, but it is reported as a possible definition. The rule accepts
  only the three functions by name, erring towards `recovered`.
- The change needed a parser-fork revision and a new `[patch.crates-io]` pin.

### Neutral

- Diagnostic locations in `recovered` reports move to the true line. Consumers
  that compared locations across versions see different values for the same
  input.

## Acceptance criteria

1. A Makefile whose read-time guards are `$(error ...)` lines inside `ifneq`
   blocks reports `complete` with exactly its real rules.
2. `$(info ...)` and `$(warning ...)` lines, with or without a trailing comment,
   report `complete` and add no fact.
3. A `$(eval ...)`, `$(foreach ...)` or bare `$(VAR)` line reports `recovered`
   with one diagnostic located on that line.
4. `schemas/makeutil.parse.v1.schema.json` is unchanged and every new fixture's
   report validates against it.
