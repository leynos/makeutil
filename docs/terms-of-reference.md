# makeutil terms of reference

- **Status:** Draft v0.1
- **Audience:** `makeutil` implementers, downstream policy authors, and
  integration reviewers
- **Companion documents:** [Technical design](design.md) and
  [ADR-0001](adrs/0001-single-file-gnu-make-parse.md)

## 1. Purpose

`makeutil` exists to turn one GNU Makefile into deterministic, source-located
structured data without evaluating or executing the file.

Its first downstream policy consumer needs enough evidence to audit required
Makefile targets and the binding of the Rust lint gate, while keeping Make
syntax and source-location handling outside Rego and Python.

These terms of reference define the problem and the ownership boundary. The
technical design chooses the implementation shape.

## 2. Domain

The domain is static, source-faithful inspection of GNU Makefiles. `makeutil`
sits between Make syntax and tools that consume ordinary JSON, including OPA,
Conftest, policy consumers, tests, and future editor integrations.

A Makefile is an executable program. Parsing can establish which rules,
variable definitions, recipes, conditionals, and include directives appear in
source. Parsing alone cannot establish every value or execution path that GNU
Make would compute.

`makeutil` therefore supplies **syntax facts with explicit limits**. It does
not claim to supply GNU Make's effective runtime model.

## 3. Users and stakeholders

| Actor                 | Role                                             | Need                                                                         |
| --------------------- | ------------------------------------------------ | ---------------------------------------------------------------------------- |
| Policy author         | Writes Rego rules over repository evidence       | Receive stable facts rather than parse Make syntax in policy.                |
| Policy orchestrator   | Runs policy against a repository checkout        | Invoke a deterministic command and associate findings with source locations. |
| `makeutil` maintainer | Maintains the parser adapter and output contract | Keep upstream parser changes behind a narrow, tested boundary.               |
| CI and fixture author | Verifies policy and parser behaviour             | Exercise representative Makefiles without running their recipes.             |

## 4. Job to be done

When a policy engine needs to inspect a Makefile, it wants a safe command that
reports explicit source constructs and their locations, so it can make a policy
decision without invoking `make`, embedding a Make parser, or relying on
regular expressions over the complete file.

For the first downstream use case, the consumer needs to answer these questions:

- Does the root Makefile explicitly define `build`, `test`, and `lint` targets?
- Does a relevant target sit inside a conditional branch?
- Does the file define a gate-critical variable such as `WHITAKER` with `?=`?
- What raw recipe text belongs to `lint` and `lint-*` targets?
- Does a recipe carry Make's ignore-errors prefix?
- Where in the original file should a finding point?

## 5. Goals

- Parse one caller-supplied UTF-8 GNU Makefile.
- Never execute GNU Make, a recipe, a Make function, or a shell command.
- Preserve the original source through the upstream lossless concrete syntax
  tree.
- Report rules, variable definitions, recipes, include directives, and their
  enclosing conditional branches.
- Report byte ranges and one-based line and column locations.
- Emit deterministic JSON under a versioned schema.
- Return parse diagnostics while retaining the recovered syntax facts that the
  upstream parser can safely expose.
- Keep `makefile-lossless` and its Rowan tree behind a `makeutil`-owned
  contract.
- Give consumers enough raw evidence to make policy decisions without placing
  policy inside `makeutil`.

## 6. Non-goals

The first release will not:

- evaluate variables, functions, pattern rules, implicit rules, prerequisites,
  or target freshness;
- execute `make -p`, `make -q`, `make -n`, or any other Make invocation;
- follow `include`, `-include`, or `sinclude` directives;
- discover Makefiles, repositories, Git refs, or Cargo workspaces;
- parse `Cargo.toml`, workflow YAML, shell syntax, or Rego;
- decide whether a repository or Makefile complies with downstream policy;
- rewrite or format a Makefile;
- expose Python, Go, C, or WebAssembly bindings;
- support BSD Make, POSIX Make, or Microsoft NMake dialects;
- promise a stable Rust library API outside the project;
- provide a daemon or long-running service.

These exclusions describe the first useful slice, not a permanent prohibition.
Each later capability requires its own evidence and decision.

## 7. Success criteria

The first release succeeds when:

- `makeutil parse Makefile` emits schema-valid JSON for representative Rust
  project Makefiles;
- complete parses reproduce the input byte-for-byte when the upstream tree is
  rendered without mutation;
- the output identifies explicit rules, recipes, variable operators, includes,
  conditional ancestry, and source locations needed by FP-003 and the first
  subset of QG-001;
- malformed input produces a recovered result with diagnostics and a distinct
  exit status rather than a false successful parse;
- two runs over identical bytes produce byte-identical compact JSON;
- parsing a file containing `$(shell ...)`, `!=`, or hostile recipe text causes
  no external process or filesystem side effect;
- fixture and golden tests cover the accepted contract;
- an external consumer can use the output without importing Rust or
  `makefile-lossless` types.

## 8. Constraints

- The implementation language is Rust because the selected parser is the Rust
  crate [`makefile-lossless`](https://github.com/jelmer/makefile-lossless).
- The first implementation pins the parser crate exactly. It does not accept an
  unconstrained `0.x` upgrade.
- The command must operate without network access.
- Standard output is reserved for the JSON result. Human diagnostics and
  invocation failures use standard error.
- Source content remains untrusted input.
- The first schema must favour facts that the parser directly supports over
  inferred semantics.
- The CLI JSON contract is the integration boundary. Internal Rust structures
  may change without notice.

## 9. Assumptions

- The initial Rust repository corpus uses UTF-8 root Makefiles.
- The first policies can work from explicit rules and raw recipe text without a
  complete GNU Make evaluator.
- A one-process invocation per repository is fast enough for the first
  downstream slice.
- The consumer will reject or mark incomplete any recovered parse rather than
  treating partial evidence as proof of compliance.
- Include traversal and source mutation can wait until the single-file contract
  has proved useful against real repositories.

## 10. Open questions deferred from the first slice

- Whether later commands should follow literal include paths.
- Whether the project should expose a source-preserving `rewrite` command.
- Whether high-volume estate scans justify a JSON Lines batch mode.
- Whether Python bindings improve consumer performance enough to justify native
  wheel distribution.
- Whether the parser adapter should recognize a restricted set of Make
  functions semantically.
- Whether a later schema should describe target delegation or an effective rule
  graph.

None of these questions blocks `makeutil parse`.

## 11. Handoff

The technical design must define the CLI, schema, traversal, diagnostics,
security properties, and verification strategy. ADR-0001 must freeze the first
slice and prevent parser work from quietly expanding into policy evaluation,
repository discovery, or rewriting.
