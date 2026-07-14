# makeutil technical design

- **Status:** Draft v0.1
- **Audience:** Implementers, maintainers, Concordat integrators, and reviewers
- **Companion documents:** [Terms of reference](terms-of-reference.md) and
  [ADR-0001](adrs/0001-single-file-gnu-make-parse.md)

## 1. Problem statement

Concordat needs structured evidence from Makefiles before OPA can enforce Rust
lint policy. Rego should not parse Make syntax, and Concordat should not invoke
GNU Make to inspect an untrusted file.

The selected `makefile-lossless` crate already provides a lossless Rowan
concrete syntax tree, recovered parse results, source ranges, and focused APIs
for rules, recipes, variables, includes, and conditionals. `makeutil` will
adapt that parser into a small, versioned JSON contract.

The first design deliberately stops at syntax facts. It does not model GNU
Make's complete evaluation semantics.

## 2. Goals and non-goals

### 2.1. Goals

- Provide `makeutil parse` for one path or standard input.
- Parse as GNU Make without executing source.
- Emit deterministic JSON with source locations.
- Flatten nested conditional content while preserving branch ancestry.
- Surface parser recovery explicitly.
- Keep the upstream CST and API out of consumer contracts.
- Support the first Concordat FP-003 and QG-001 policy slice.

### 2.2. Non-goals

- File or repository discovery.
- Include traversal.
- Variable or function evaluation.
- Shell parsing.
- Compliance decisions.
- Makefile mutation.
- Cargo or workflow parsing.
- Language bindings.
- Dialects other than GNU Make.

## 3. Design principles

### 3.1. Parse, do not execute

No production path may invoke GNU Make or a shell. Text such as `$(shell ...)`
and `VAR != command` remains inert source data.

### 3.2. Report evidence, not conclusions

`makeutil` reports that a variable uses `?=` or that a recipe starts with `-`.
Concordat and Rego decide whether those facts violate policy.

### 3.3. Preserve uncertainty

A recovered parse is not a complete parse. The output records diagnostics and
uses a distinct status and process exit code. Consumers must not convert
partial evidence into a compliance pass.

### 3.4. Own the integration contract

The JSON schema belongs to `makeutil`. It must not serialize Rowan nodes or
copy the upstream AST shape. An upstream crate upgrade may alter the adapter,
but it must not silently alter schema version 1.

### 3.5. Keep the first command narrow

One command reads one file and writes one document. Discovery, batching,
rewriting, and bindings remain later decisions.

## 4. External dependency

The implementation uses
[`makefile-lossless`](https://github.com/jelmer/makefile-lossless), initially
pinned to `=0.3.40`. A temporary `[patch.crates-io]` override selects commit
`8dd35801b75b332c2ac2f995ae398ef8238559fa` from the `leynos/makefile-lossless`
fork because release 0.3.40 does not lex the documented GNU Make `!=`
assignment operator. Remove the override when an upstream release containing
the fix is adopted; do not replace the immutable commit with a branch name.

The crate supplies:

- a lossless CST that retains whitespace, comments, and formatting;
- a recovered parse tree alongside positioned errors;
- rule targets, prerequisites, recipes, and source ranges;
- variable names, assignment operators, raw values, and directive flags;
- include paths and optionality;
- GNU Make conditional structure and nested items.

`makeutil` does not rely on the crate for full Make evaluation. In particular,
static variable expansion and complete Make function semantics remain outside
this design.

An upgrade to the parser crate requires the complete fixture, golden, and
round-trip test suite to pass. The dependency stays exact until the project has
sufficient compatibility evidence to choose another policy.

## 5. Command-line contract

### 5.1. Commands

The first release exposes one command:

```shell
makeutil parse PATH
```

A single dash reads source from standard input. The caller must provide the
logical source path used in diagnostics:

```shell
makeutil parse --stdin-filename Makefile -
```

No implicit path, directory walk, glob, or recursive mode exists in the first
slice.

### 5.2. Output streams

- Standard output contains exactly one compact JSON document for a successful or
  recovered parse.
- Standard error contains invocation, I/O, UTF-8, serialization, and internal
  failures.
- The command does not emit progress text.

### 5.3. Exit codes

| Code | Meaning                                                                                                  |
| ---- | -------------------------------------------------------------------------------------------------------- |
| `0`  | The source parsed without diagnostics and JSON was emitted.                                              |
| `1`  | The parser recovered a tree with one or more diagnostics and JSON was emitted.                           |
| `2`  | Invocation, source reading, UTF-8 decoding, serialization, or internal failure prevented a parse result. |

The distinct recovered status lets Concordat fail closed while retaining useful
source diagnostics.

## 6. Output model

### 6.1. Top-level document

Schema version 1 has this shape:

```json
{
  "schema_version": 1,
  "tool": {
    "name": "makeutil",
    "version": "0.1.0",
    "parser": "makefile-lossless",
    "parser_version": "0.3.40"
  },
  "source": {
    "path": "Makefile",
    "sha256": "…",
    "byte_length": 1234
  },
  "parse": {
    "status": "complete",
    "diagnostics": []
  },
  "rules": [],
  "variables": [],
  "includes": []
}
```

`parse.status` is either `complete` or `recovered`.

The schema does not include the complete source text or CST. The caller already
owns the source, and duplicating it would enlarge policy input without adding
facts.

### 6.2. Source locations

All facts and conditional contexts that originate from source carry the same
complete location shape:

```json
{
  "start_byte": 42,
  "end_byte": 67,
  "start_line": 4,
  "start_column": 1,
  "end_line": 5,
  "end_column": 1
}
```

Byte ranges use a zero-based, end-exclusive convention. Lines and columns are
one-based for diagnostic consumers. Columns count UTF-8 bytes, matching Rowan's
offsets and avoiding an unadvertised character-width conversion.

Locations own these source ranges:

- A rule covers the complete rule node, from the first target byte through the
  final recipe byte and line ending when one exists.
- A recipe covers its complete physical source, including the leading recipe
  tab, any `@`, `-`, or `+` modifiers, continuations, and final line ending.
- A variable covers the complete definition or directive, including modifiers,
  assignment operator, value, and final line ending.
- An include covers the complete include directive and final line ending.
- A conditional context covers only its opening `ifdef`, `ifndef`, `ifeq`, or
  `ifneq` directive, or its `else` directive, including that directive's final
  line ending. It does not cover the nested arm or closing `endif`.
- A diagnostic uses the upstream positioned range. When only a line is known,
  it covers that complete line excluding its line ending. An end-of-file
  diagnostic is a zero-length range at `byte_length`.

A range must satisfy `start_byte <= end_byte <= source.byte_length`. Empty and
zero-length ranges are valid insertion points. End-of-file positions identify
the point immediately after the final byte. In CRLF input, the carriage return
and line feed remain bytes on the preceding line; the next line begins after
the line feed.

### 6.3. Parse diagnostics

A positioned parse diagnostic contains:

```json
{
  "message": "expected ':'",
  "code": null,
  "location": {
    "start_byte": 12,
    "end_byte": 15,
    "start_line": 2,
    "start_column": 1,
    "end_line": 2,
    "end_column": 4
  }
}
```

The adapter uses positioned diagnostics when available. It derives a line-level
range from the upstream error information only when no positioned range exists.

### 6.4. Conditional context

Rules, variables, and includes inside conditionals carry their complete
outer-to- inner context:

```json
[
  {
    "kind": "ifdef",
    "expression": "CI",
    "branch": "if",
    "location": {
      "start_byte": 120,
      "end_byte": 129,
      "start_line": 10,
      "start_column": 1,
      "end_line": 11,
      "end_column": 1
    }
  }
]
```

`branch` is `if` or `else`. The first slice does not evaluate the expression.

### 6.5. Rule facts

```json
{
  "ordinal": 7,
  "targets": ["lint"],
  "prerequisites": [],
  "double_colon": false,
  "conditions": [],
  "recipes": [
    {
      "ordinal": 0,
      "text": "$(WHITAKER) --all -- $(CARGO_FLAGS)",
      "silent": false,
      "ignore_errors": false,
      "always_execute": false,
      "location": {}
    }
  ],
  "location": {}
}
```

Recipe `text` excludes the leading recipe prefix tab and trailing line ending.
It retains internal whitespace, comments, continuations, variable references,
and shell syntax exactly as exposed by the parser.

The prefix booleans describe Make's `@`, `-`, and `+` recipe modifiers. They do
not describe shell operators such as `|| true`; policy may inspect the raw text
for the first bounded rules.

### 6.6. Variable facts

```json
{
  "ordinal": 4,
  "name": "WHITAKER",
  "operator": "?=",
  "raw_value": "whitaker",
  "exported": false,
  "overridden": false,
  "define_block": false,
  "conditions": [],
  "location": {}
}
```

The operator remains source-faithful. The first slice does not calculate the
effective value or precedence.

### 6.7. Include facts

```json
{
  "ordinal": 2,
  "raw_path": "$(CONFIG_DIR)/rules.mk",
  "optional": false,
  "dynamic": true,
  "conditions": [],
  "location": {}
}
```

`dynamic` is true when the raw include expression contains a Make variable or
function marker. `makeutil` reports includes but never opens them.

## 7. Internal architecture

The first slice is implemented by `domain`, `ports`, `application`, and
`adapters` modules in one crate. This is a boundary protection measure, not a
pattern transplant: `MakefileParser` is the sole port because the upstream CST
is the sole volatile external semantic boundary. Source input, JSON, and CLI
code remain ordinary edge adapters. The source adapter accepts a narrow
`SourceReader` capability interface so it can classify open and read failures
without resolving ambient authority. The CLI composition boundary constructs
the concrete ambient-backed reader once and injects it downwards; this
interface is an adapter test seam, not a domain port.

The parser port is owned by the domain and called only by `parse_source`.
Adapter implementations may compose upstream accessors and Rowan ranges, but
must return only `SyntaxObservation` values. Those observations are not a
second public schema and must not be consumed directly by the CLI. New callers
compose through `parse_source`, which owns validation, hashing, locations,
ordinals, and status.

The direct `rowan` dependency exists only to bring its `AstNode` trait into the
parser adapter for upstream syntax ranges. `data-encoding` owns lower-case
digest rendering. Neither dependency expands the stable public contract.

| Component      | Responsibility                                                                    |
| -------------- | --------------------------------------------------------------------------------- |
| CLI front end  | Parse the command, validate one source, and compose ambient process capabilities. |
| Source reader  | Read one injected path capability or stdin without interpreting or normalizing.   |
| Parser adapter | Invoke `makefile-lossless` and return ordered owned observations and diagnostics. |
| Fact collector | Flatten observations, attach conditions and locations, and assign ordinals.       |
| Location index | Convert byte offsets into one-based line and byte-column positions.               |
| JSON reporter  | Serialize schema version 1 deterministically to standard output.                  |

The package may expose a Rust library internally for unit tests, but only the
CLI and JSON schema form a supported integration contract in the first release.

The domain owns report types, source spans and locations, conditional ancestry,
ordinal assignment, diagnostic ordering, source hashing, and complete versus
recovered classification. A domain-owned parser port accepts UTF-8 text and
returns ordered makeutil-owned syntax observations, source spans, and
diagnostics. The `makefile-lossless` adapter implements the port and proves its
own complete-tree round trip; it never returns Rowan nodes, upstream errors, or
rendered CST bytes through the port.

The application service calculates SHA-256 over the exact input bytes while
`parse_source` constructs `SourceIdentity`.

The composition root parses the CLI, invokes the source reader, calls the
application service, and hands the completed report to the JSON reporter. Edge
adapters do not call each other. Source and reporter traits are introduced only
when deterministic failure testing requires them; they are not domain ports.

## 8. Parse and traversal flow

1. Read the source bytes.
2. Reject non-UTF-8 input with exit code 2.
3. Calculate the source digest and line-start index.
4. Parse with the GNU Make default of `makefile-lossless`.
5. Obtain the tree even when parser diagnostics exist.
6. Walk root items in source order.
7. Recurse into the `if` and `else` arms of each conditional while extending the
   condition context.
8. Emit flattened rule, variable, and include facts with global source-order
   ordinals.
9. Serialize one compact JSON document.
10. Return 0 for a complete parse or 1 for a recovered parse.

The collector does not walk included files or attempt to merge repeated target
rules into an effective rule.

## 9. Determinism

- Rules, variables, and includes have separate arrays, but their top-level
  `ordinal` values share one zero-based, gap-free sequence ordered by
  `location.start_byte`. Equal offsets retain upstream observation order.
  Recipe ordinals are zero-based and local to their containing rule.
- Diagnostics retain upstream emission order; the adapter does not sort,
  deduplicate, or rewrite messages or codes.
- Object fields use a fixed serialization order derived from Rust structures.
- Compact JSON ends with one newline.
- The source path preserves the caller-supplied UTF-8 spelling byte-for-byte.
  It is neither canonicalized nor lexically cleaned. In stdin mode it is
  exactly the value of `--stdin-filename`.
- The digest covers the exact input bytes.
- No timestamps, hostnames, temporary paths, or environment values appear in
  output.

## 10. Security properties

- The command never invokes GNU Make.
- The command never invokes a shell.
- It performs no variable or function expansion.
- It opens only the explicitly supplied input path.
- It does not follow includes or symlinks discovered from source.
- It performs no network access.
- Source text cannot select another parser, command, or output path.
- Resource limits may be added later if corpus evidence shows pathological
  inputs; the first slice still includes large-file and deep-conditional tests.

The security suite uses source-selected filesystem sentinels for `$(shell ...)`,
`$(file ...)`, `!=`, and recipes. It separately traces file-open system calls
and proves that literal and dynamic include paths are not opened; absence of a
side effect alone is not evidence that an include was not read.

### 10.1. Failure and observability contract

The reporter serializes the complete JSON document into memory before writing
stdout. Serialization failure therefore emits no JSON. An output write failure,
including a broken pipe, exits 2. The operating system may already have
accepted a prefix of the buffered document, so partial stdout is permitted only
for this failure class.

Fatal stderr diagnostics have one stable first line:

```plaintext
makeutil: <operation-id>: <detail>
```

Operation identifiers distinguish `cli`, `source-open`, `source-read`,
`source-utf8`, `parse-internal`, `json-serialize`, and `stdout-write`. Normal
success and recovered parsing emit no stderr. The detail includes the logical
path for `source-open` and `source-read` failures. Backtraces and cause chains
are not printed by default. The binary may install one tracing subscriber, but
it must never write tracing events to stdout; the library installs no
subscriber. Source contents and unbounded raw paths are not tracing fields.
This one-shot CLI emits no metrics in the first slice.

## 11. Verification strategy

### 11.1. Fixture classes

The repository will include focused Makefiles for:

- explicit `build`, `test`, and `lint` targets;
- multiple targets on one rule;
- repeated and double-colon rules;
- `WHITAKER ?= whitaker` and other assignment operators;
- exported and overridden variables;
- `lint` and `lint-*` recipe text;
- `@`, `-`, and `+` recipe prefixes;
- multiline recipes and continuations;
- nested `ifdef`, `ifndef`, `ifeq`, and `ifneq` branches;
- literal, optional, and dynamic includes;
- parse errors with recovered facts;
- inert `$(shell ...)`, `!=`, and hostile shell text.

### 11.2. Test types

- **Unit tests:** source location conversion and fact extraction.
- **Golden tests:** complete JSON output for representative fixtures.
- **Round-trip tests:** complete parses render exactly the original bytes.
- **Determinism tests:** repeated invocation produces byte-identical output.
- **No-execution tests:** sentinel source cannot create files or processes.
- **Compatibility tests:** parser upgrades run against the complete fixture
  corpus before the dependency pin changes.

The first slice does not require a new fuzzing campaign because the upstream
crate already owns parser fuzzing. `makeutil` may add fuzzing later for the
adapter and location mapping if real corpus defects justify it.

## 12. Packaging and integration

The project ships one Rust binary named `makeutil`.

Concordat invokes the binary as a subprocess and validates `schema_version`.
The first slice does not expose PyO3 or Go bindings. This avoids a native wheel
matrix, cgo, and direct consumer coupling to upstream Rust types.

CI and release packaging pin the executable version. The JSON schema, rather
than the executable version string alone, controls compatibility.

## 13. Deferred extension seams

The design leaves room for later additions without implementing them now:

- `makeutil rewrite` using semantic mutation requests and the same lossless CST;
- literal include traversal supplied by the caller as an explicit file set;
- JSON Lines batch input for estate throughput;
- a Python transactional binding if subprocess cost becomes material;
- additional fact collectors for a restricted Make semantic subset.

A later command must not silently change the behaviour or schema of `parse`.

## 14. Risks and mitigations

| Risk                                                        | Mitigation                                                                                            |
| ----------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Consumers mistake syntax facts for effective Make semantics | Name fields literally, document the single-file scope, and preserve conditional and include evidence. |
| Upstream `0.x` changes alter behaviour                      | Pin exactly and require fixture, golden, and round-trip review for upgrades.                          |
| Recovered parses lead to false passes                       | Emit `parse.status = recovered`, exit 1, and require Concordat to fail closed.                        |
| Raw recipe matching becomes policy-specific parsing         | Keep matching in Rego and limit the first rules to documented lexical patterns.                       |
| Schema expands before evidence exists                       | Require a consumer use case and schema-version review for every new fact.                             |

## 15. First-slice acceptance criteria

The design is implemented when:

1. `makeutil parse PATH` and stdin mode work as specified.
2. Schema version 1 is documented and covered by golden tests.
3. Complete input round-trips byte-for-byte through the parser tree.
4. Recovered input emits diagnostics, partial facts, and exit code 1.
5. Rules, variables, recipes, includes, conditional ancestry, and source
   locations cover the first Concordat fixtures.
6. No test source can cause command execution or network access.
7. Concordat can consume the output solely through JSON.
