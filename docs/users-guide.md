# User guide

This guide explains how to parse one GNU Makefile into source-faithful JSON
facts with `makeutil`.

Integrations upgrading from the greeting scaffold should follow the
[version 0.1.0 migration guide](v0-1-0-migration-guide.md).

## Parse a file

Pass exactly one UTF-8 path to the `parse` subcommand:

```shell
makeutil parse Makefile
```

The command writes one compact JSON document followed by a newline. It reports
explicit rules, recipes, variable definitions, include directives, conditional
ancestry, source locations, and parser diagnostics. It does not evaluate Make
expressions, run recipes or shell functions, or open reported include paths.

## Parse standard input

Use `-` for standard input and supply the logical path recorded in the report:

```shell
makeutil parse --stdin-filename Makefile - < Makefile
```

`--stdin-filename` is required with `-` and rejected for file paths. These
arguments are command-line-only: environment variables and configuration files
cannot supply them.

Path and standard-input sources may contain at most 16 MiB (16,777,216 bytes).
The limit is inclusive. A larger source fails before parsing with exit code 2,
writes a `makeutil: source-too-large: DETAIL` diagnostic to standard error, and
emits no JSON.

## Interpret results

The normative output contract is
[`schemas/makeutil.parse.v1.schema.json`](../schemas/makeutil.parse.v1.schema.json).
Byte ranges are zero-based and end-exclusive. Display lines and byte columns
are one-based.

| Exit code | Meaning                                                                |
| --------- | ---------------------------------------------------------------------- |
| `0`       | Parsing completed and JSON was emitted.                                |
| `1`       | Parsing recovered partial facts with diagnostics and JSON was emitted. |
| `2`       | Invocation, input, UTF-8, internal, serialization, or output failed.   |

_Table 1: `makeutil parse` exit codes._

Fatal failures write a stable `makeutil: OPERATION: DETAIL` diagnostic to
standard error and do not intentionally emit JSON. Control characters in
caller-supplied paths are escaped in this diagnostic, so its first line cannot
be forged. The JSON report preserves the exact caller-supplied logical path.
Recovered reports are insufficient proof that a Makefile is compliant.

### Bare `export` directives

A bare `export NAME` directive names a variable assigned elsewhere, so it
carries no assignment operator and no value. Such a directive appears in the
`variables` array with `operator` set to the empty string, `raw_value` set to
the empty string, `exported` set to `true`, and `define_block` set to `false`.
A consumer that wants only genuine assignments should therefore filter on a
non-empty `operator`; the predicate `operator == "" && define_block == false`
identifies a bare export directive rather than an assignment. A directive
naming several variables yields one entry per name.

The empty operator is shared with `define` blocks, which is why `define_block`
is part of the predicate. An `export NAME := value` line is an ordinary
assignment: it reports its real operator and its value, with `exported` set to
`true`.

### `unexport` is not yet supported

An `unexport` directive is currently reported as a rule whose first target is
the word `unexport`, and it forces a `recovered` status with an `expected ':'`
diagnostic. Schema version 1 has no way to express "this name was explicitly
un-exported", so faithful support awaits a schema version that can. Treat any
`unexport` line in a report as an unrepresented construct rather than as a real
rule.
