# Polonius migration

This project adopts the location-sensitive Polonius borrow-checking analysis
provided by `-Zpolonius=next`. The repository pins its nightly compiler in
`rust-toolchain.toml`, and `.cargo/config.toml` supplies the flag to Cargo and
rust-analyzer.

Commands that set `RUSTFLAGS` replace Cargo's configured flags. The Makefile
and continuous-integration coverage job therefore repeat `-Zpolonius=next`
explicitly. Use plain `cargo` commands from this repository so the pinned
toolchain is selected; an unpinned `cargo +nightly` may not include the
Cranelift component required by the development profile.

## Design rules

- Prefer a single-lookup get-or-create function that returns `&mut V` and
  clones an owned key only on a miss.
- Return references from internal lookup and traversal APIs. Keep ids when
  they are persisted, serialized, compared as identity, or cross a thread or
  process boundary.
- Mutate values in place instead of cloning, modifying, and writing them back.
- Build error context lazily in the failure branch.
- Keep owned values where simultaneous aliasing, suspension points, thread or
  process boundaries, or struct-field lifetimes require them. Polonius changes
  loan liveness, not Rust's aliasing rules.
- Keep loop restructures needed for loop-carried conditional reborrows; the
  current Polonius analysis does not accept full flow-sensitive forms.

Borrow-sensitive sites use one of these greppable comments:

- `POLONIUS(case-3)`, `POLONIUS(lending-iter)`, or
  `POLONIUS(scan-mutate)` marks code that intentionally requires Polonius.
- `POLONIUS-CANDIDATE(...)` marks a deferred rewrite.
- `POLONIUS-REFUSED(...)` records a permanent ownership constraint such as
  aliasing, a suspension point, identity data, or flow sensitivity.

## Audit inventory

The initial audit used nightly `1.98.0-nightly (57d06900f 2026-05-27)` on
13 July 2026. Both `cargo check` and `RUSTFLAGS="-Zpolonius=next" cargo check`
passed. The repository contained no borrow-checker workarounds or owned-value
API pressure to rewrite.

### Rewritten sites

| File | Pattern                     | Verified   | Nightly        |
| ---- | --------------------------- | ---------- | -------------- |
| None | No rewrite candidates found | 2026-07-13 | 1.98.0-nightly |

### API evolution targets

| Owning API | Target signature                | Shape          | Status  |
| ---------- | ------------------------------- | -------------- | ------- |
| None       | No lookup or traversal APIs yet | Not applicable | Audited |

### Candidates awaiting stabilization

| File | Candidate           | Status  |
| ---- | ------------------- | ------- |
| None | No candidates found | Audited |

### Refused rewrites

| File | Constraint           | Rationale |
| ---- | -------------------- | --------- |
| None | No refusals recorded | Audited   |
