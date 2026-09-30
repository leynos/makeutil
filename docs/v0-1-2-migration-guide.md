# Migrate to makeutil 0.1.2

Version 0.1.2 reads bare expansion lines, such as `$(error ...)` read-time
guards, instead of misreading them as rules, and places every parser diagnostic
on the line it concerns. `schema_version` stays at `1` and the schema file is
unchanged.

## Guards no longer invent rules

In 0.1.1, a top-level `$(info ...)`, `$(warning ...)` or `$(error ...)` line
was parsed as a rule missing its colon. Each one added a rule to `rules` and an
`expected ':'` diagnostic, and made the report `recovered`. In 0.1.2 such a
line adds nothing, and a Makefile whose only unusual lines are these reports
`complete` with only its real rules.

A consumer that counted rules, or that treated these reports as unparsable,
sees fewer rules and a `complete` status for the same input.

## Other expansion lines name themselves

A `$(eval ...)`, `$(call ...)`, `$(foreach ...)` or bare `$(VAR)` line may
define rules or variables that a static parse cannot see. It remains
`recovered`, but its diagnostic now reads
`expansion line may define rules or variables a static parse cannot see` and
covers that line, instead of `expected ':'` elsewhere. See
[Bare expansion lines](users-guide.md#bare-expansion-lines) in the users' guide
and [ADR-0003](adrs/0003-bare-expansion-lines.md).

## Diagnostic locations moved

Every `recovered` report's diagnostics now point at the offending line. In
0.1.1 one channel always reported the end of input and the other named an
unrelated token. Consumers that compared diagnostic locations across versions
see different values for the same input.
