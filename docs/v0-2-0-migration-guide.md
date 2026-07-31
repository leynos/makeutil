# Migrate to makeutil 0.2.0

Version 0.2.0 replaces the generated greeting scaffold with a command that
parses one GNU Makefile into versioned JSON facts.

## Remove the greeting API

Remove imports and calls to `makeutil::greet`. Version 0.2.0 does not provide a
replacement library function. Integrations should invoke the `makeutil`
executable and consume its versioned JSON output instead.

## Invoke the parser

Replace greeting invocations with the path form when the Makefile is stored on
disk:

```shell
makeutil parse Makefile
```

Use standard input only when the integration already owns the source bytes:

```shell
makeutil parse --stdin-filename Makefile - < Makefile
```

The input path and `--stdin-filename` are command-line-only values.
Configuration files and environment variables cannot supply them.

## Update JSON consumers

Validate output against
[`schemas/makeutil.parse.v1.schema.json`](../schemas/makeutil.parse.v1.schema.json)
and require `schema_version` to equal `1`. Exit status `0` emits a complete
report. Exit status `1` emits a recovered report with diagnostics. Exit status
`2` denotes a fatal invocation, input, serialization, or output failure and
does not intentionally emit JSON.

See the [user guide](users-guide.md) for the complete command, stream, and
source-location contracts.
