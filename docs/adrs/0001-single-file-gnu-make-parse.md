# ADR-0001: Parse one GNU Makefile into versioned JSON facts

## Status

Accepted on 2026-07-13

## Context

A downstream policy consumer needs to inspect root Makefiles for required
targets and known lint-gate bypasses. OPA and Rego consume structured data;
they should not parse Make syntax. Invoking GNU Make to inspect an untrusted
file would also cross the static-analysis boundary because Make expands
functions and may invoke external commands while reading source.

The Rust crate
[`makefile-lossless`](https://github.com/jelmer/makefile-lossless) supplies a
lossless concrete syntax tree, recovered parse results, source ranges, and
focused accessors for the constructs needed by the first policy.

The main risk is accidental expansion. A seemingly small parser wrapper could
become a repository scanner, Make evaluator, policy engine, mutation service,
and multi-language binding project before it proves its first use case.

## Decision

The first `makeutil` slice will ship one Rust CLI command:

```shell
makeutil parse PATH
```

It will:

- accept exactly one UTF-8 GNU Makefile by path or stdin;
- use an exactly pinned `makefile-lossless` dependency, initially `=0.3.40`;
- parse without invoking GNU Make, a shell, or any source-selected command;
- emit versioned, deterministic JSON;
- report explicit rules, prerequisites, recipes, recipe prefixes, variable
  definitions, assignment operators, include directives, and conditional
  ancestry;
- attach byte ranges and one-based line and byte-column locations;
- emit recovered facts and parser diagnostics with exit code 1 when parsing is
  incomplete;
- reserve exit code 2 for invocation, I/O, UTF-8, serialization, and internal
  failures;
- treat the CLI JSON schema as the only stable integration contract.

The first slice will not:

- discover files or repositories;
- follow include directives;
- evaluate variables, Make functions, pattern rules, or target graphs;
- parse shell, Cargo, YAML, or policy files;
- make compliance decisions;
- edit or write Makefiles;
- expose Python, Go, C, or WebAssembly bindings;
- provide batch or daemon modes;
- support a Make dialect other than GNU Make.

A complete parse must round-trip byte-for-byte through the upstream lossless
tree. A consumer must treat a recovered parse as insufficient proof of
compliance.

## Slice boundary

| Concern     | First slice                | Later decision                           |
| ----------- | -------------------------- | ---------------------------------------- |
| Input       | One explicit file or stdin | Discovery and batches                    |
| Dialect     | GNU Make                   | BSD, POSIX, NMake                        |
| Semantics   | Source syntax facts        | Restricted evaluation or effective graph |
| Includes    | Report only                | Caller-supplied traversal                |
| Output      | Compact JSON schema v1     | JSON Lines or other reporters            |
| Mutation    | None                       | Source-preserving rewrite command        |
| Integration | Subprocess CLI             | Optional Python or other bindings        |
| Policy      | None                       | Remains owned by consumers and Rego      |

## Consequences

### Positive

- The downstream policy consumer gets the evidence it needs without owning a
  Make parser.
- Rego receives ordinary structured data with source locations.
- Untrusted Makefile content remains inert.
- The adapter isolates consumers from a young upstream crate's Rust API.
- The narrow command can be tested against the real estate before mutation or
  remote execution raises the stakes.

### Negative

- A subprocess adds packaging and invocation overhead.
- Single-file syntax facts cannot prove behaviour hidden in includes, dynamic
  expansion, or generated rules.
- Consumers need an explicit policy for recovered parses and unsupported scope.
- Exact dependency pinning requires deliberate upgrade work.

### Neutral

- The lossless writer remains available upstream, but this decision does not
  expose it.
- The first JSON schema may need additive fields after real repository trials.
  Incompatible changes require a new schema version.

## Alternatives considered

### Call `make -p`, `make -q`, or `make -n`

Rejected. GNU Make may expand functions and execute shell commands while
reading source. It also reports an evaluated database rather than the
source-faithful facts needed for precise diagnostics and later editing.

### Parse Makefiles in Python inside the consumer

Rejected. This would duplicate a difficult grammar, lose the proven lossless
CST, and bind parser maintenance to the consumer.

### Bind the Rust crate directly into Python now

Rejected for the first slice. PyO3 would add native wheel distribution and
couple the consumer directly to Rust packaging before subprocess overhead has
shown itself to matter.

### Expose a C ABI for Go or OpenTofu integration

Rejected. It would introduce ownership, panic, linking, and cross-compilation
complexity without improving the first local audit.

### Implement parsing and rewriting together

Rejected. Reliable findings and a stable fact model must precede automated
edits. The writer deserves a separate mutation vocabulary, safety invariants,
and decision.

## Acceptance criteria

1. The command implements the documented input, output, and exit-code contract.
2. Complete parses round-trip byte-for-byte.
3. Golden output covers the facts needed by FP-003 and the first subset of
   QG-001.
4. Recovered parses never exit 0.
5. Hostile Make functions and recipes cause no external side effect.
6. An external consumer can use the JSON output without a language binding.
7. Include traversal, mutation, discovery, and policy remain absent from the
   implementation.
