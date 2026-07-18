# User guide

This guide explains how to parse one GNU Makefile into source-faithful JSON
facts with `makeutil`.

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
standard error and do not intentionally emit JSON. Recovered reports are
insufficient proof that a Makefile is compliant.
