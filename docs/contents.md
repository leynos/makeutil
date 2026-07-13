# Documentation contents

[Documentation contents](contents.md) is the index for makeutil's documentation
set.

## Project guides

- [User guide](users-guide.md) explains how to use the generated project and
  its public build and test commands.
- [Developer guide](developers-guide.md) explains the local workflow and
  implementation tooling for contributors.
- [Repository layout](repository-layout.md) explains the generated project's
  top-level files, directories, and ownership boundaries.
- [Polonius migration](polonius.md) records the compiler requirement,
  borrow-centric design rules, and audit inventory.
- [Documentation style guide](documentation-style-guide.md) defines the
  spelling, structure, Markdown, Architecture Decision Record (ADR), Request
  for Comments (RFC), and roadmap conventions used by this documentation set.

## Architecture and design

- [Terms of reference](terms-of-reference.md) defines the problem space,
  stakeholders, scope, constraints, and success criteria for `makeutil`.
- [Technical design](design.md) describes the command contract, data model,
  architecture, security boundaries, and verification strategy.
- [ADR-0001: Parse one GNU Makefile into versioned JSON facts](adrs/0001-single-file-gnu-make-parse.md)
  records the proposed boundary for the first implementation slice.
- [Execution plans](execplans/) describe approved, milestone-oriented delivery
  work:
  - [Implement ADR-0001](execplans/adr-0001-single-file-gnu-make-parse.md)
    plans the single-file GNU Make parser and its verification.

## Rust reference material

- [Reliable testing in Rust via dependency injection](reliable-testing-in-rust-via-dependency-injection.md)
  explains how to keep tests deterministic by injecting environment, clock,
  filesystem, and other external dependencies.
- [Rust doctest Don't Repeat Yourself guide](rust-doctest-dry-guide.md)
  explains how to write maintainable, executable Rust documentation examples.
- [Rust testing with `rstest` fixtures](rust-testing-with-rstest-fixtures.md)
  explains fixture-based, parameterized, and asynchronous testing with `rstest`.
- [`rstest-bdd` user's guide](rstest-bdd-users-guide.md) explains how to write
  and run Behaviour-Driven Development scenarios, step definitions, and
  fixtures with `rstest-bdd`.
- [OrthoConfig user's guide](ortho-config-users-guide.md) documents the
  configuration and command-line derivation library used by the CLI adapter.

## Engineering practice

- [Complexity antipatterns and refactoring strategies](complexity-antipatterns-and-refactoring-strategies.md)
  explains cognitive complexity, the bumpy-road antipattern, and refactoring
  approaches for maintainable code.
- [Scripting standards](scripting-standards.md) explains the preferred Python
  scripting stack, command execution patterns, and test expectations for helper
  scripts.
