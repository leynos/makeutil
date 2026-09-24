//! Bare `export` regressions for directive shapes found in real Makefiles.
//!
//! `makeutil parse` once exited 2 on any bare `export NAME`, losing the whole
//! report. These fixtures keep the surroundings the directive had in the
//! Makefiles that exposed it: an empty conditional assignment exported on the
//! very next line, and two variables exported by consecutive directives. The
//! shape of assignments followed by a block of exports is `bare-export.mk`,
//! covered in `export_directives.rs`.

use makeutil::{
    adapters::MakefileLosslessParser,
    domain::{AssignmentOperator, ParseReport, ParseStatus},
    parse_source,
};
use pretty_assertions::assert_eq;
use rstest::rstest;

/// Identifying fields of one variable fact: name, operator, raw value,
/// export flag and `define_block`.
type VariableSummary<'a> = (&'a str, AssignmentOperator, &'a str, bool, bool);

/// Summarize every variable fact, in report order.
///
/// The raw value and `define_block` are kept because together with the
/// operator they are what tells an export-directive fact from an assignment.
fn variable_summary(report: &ParseReport) -> Vec<VariableSummary<'_>> {
    report
        .variables
        .iter()
        .map(|variable| {
            (
                variable.name.as_str(),
                variable.operator,
                variable.raw_value.as_str(),
                variable.exported,
                variable.define_block,
            )
        })
        .collect()
}

/// First target of every rule, in report order.
fn rule_targets(report: &ParseReport) -> Vec<&str> {
    report
        .rules
        .iter()
        .filter_map(|rule| rule.targets.first().map(String::as_str))
        .collect()
}

/// Each shape parses `complete` with no diagnostics, keeps every
/// assignment and rule, and adds one exported, valueless fact per directive
/// after the assignment it names.
#[rstest]
#[case::export_after_empty_conditional(
    include_bytes!("fixtures/makefiles/export-after-empty-conditional.mk").as_slice(),
    "export-after-empty-conditional.mk",
    vec![
        ("CRATE", AssignmentOperator::Conditional, "example", false, false),
        (
            "FORMAL_STUB",
            AssignmentOperator::Conditional,
            "./scripts/formal-stub.sh",
            false,
            false,
        ),
        ("FORMAL_STRICT", AssignmentOperator::Conditional, "", false, false),
        ("FORMAL_STRICT", AssignmentOperator::Define, "", true, false),
    ],
    vec!["build", "release", "all"],
)]
#[case::consecutive_bare_exports(
    include_bytes!("fixtures/makefiles/consecutive-bare-exports.mk").as_slice(),
    "consecutive-bare-exports.mk",
    vec![
        ("PG_PASSWORD", AssignmentOperator::Conditional, "embedded_test", false, false),
        (
            "POSTGRESQL_RELEASES_URL",
            AssignmentOperator::Conditional,
            "https://github.com/theseus-rs/postgresql-binaries",
            false,
            false,
        ),
        ("PG_PASSWORD", AssignmentOperator::Define, "", true, false),
        ("POSTGRESQL_RELEASES_URL", AssignmentOperator::Define, "", true, false),
    ],
    vec!["docs-check"],
)]
fn bare_export_shape_parses_complete(
    #[case] source: &[u8],
    #[case] path: &str,
    #[case] expected_variables: Vec<VariableSummary<'_>>,
    #[case] expected_rules: Vec<&str>,
) {
    let report = parse_source(source, path, &MakefileLosslessParser)
        .expect("a bare-export shape fixture must parse into a report");

    assert_eq!(report.parse.status, ParseStatus::Complete);
    assert_eq!(report.parse.diagnostics, Vec::new());
    assert_eq!(variable_summary(&report), expected_variables);
    assert_eq!(rule_targets(&report), expected_rules);
}
