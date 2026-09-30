# Migrate to makeutil 0.1.1

Version 0.1.1 reports GNU Make's `unexport` directive as a fact instead of a
misread rule. `schema_version` stays at `1` and the schema file is unchanged,
but consumers that read export directives must now read one more field.

## Read `exported` on directive entries

In 0.1.0, an `unexport NAME` line was parsed as a rule whose first target was
the word `unexport`. The report was `recovered`, and its two `expected ':'`
diagnostics pointed at the wrong lines. In 0.1.1 the same line reports
`complete` and adds one entry per name to `variables`, shaped like an
`export NAME` entry except that `exported` is `false`:

```json
{"name": "NAME", "operator": "", "raw_value": "", "exported": false,
 "define_block": false}
```

The predicate `operator == "" && define_block == false` still identifies a
directive, but it now matches both kinds. In 0.1.0 every directive entry had
`exported` set to `true`, so a consumer could treat the predicate alone as
"this name is exported". From 0.1.1 it must also read `exported`: `true` for
`export NAME` and `false` for `unexport NAME`.

`unexport NAME = value` is an ordinary assignment with `exported` set to
`false`. A bare `unexport` with no names produces a `recovered` report with a
diagnostic, as a bare `export` does. See
[`unexport` directives](users-guide.md#unexport-directives) in the users' guide
and the amendment to
[ADR-0002](adrs/0002-bare-export-directive-representation.md#amendment-unexport-directives).

## Parser behaviour that changed with it

The parser revision that learned `unexport` also corrected three readings of
`export` lines, to match GNU Make:

- `export FOO # comment` now parses complete; 0.1.0 reported a missing
  assignment operator.
- `override export override` now reports a variable called `override`; 0.1.0
  reported no name and a `recovered` parse.
- `export = 1` assigns a variable called `export`, which is not exported.
