//! Pure domain and application contract tests.

use makeutil::{
    adapters::MakefileLosslessParser,
    domain::{LocationIndex, ParseStatus, SourceSpan},
    parse_source,
};
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use rstest::rstest;

#[rstest]
#[case("", SourceSpan { start: 0, end: 0 }, (1, 1, 1, 1))]
#[case("a\nb", SourceSpan { start: 2, end: 3 }, (2, 1, 2, 2))]
#[case("a\r\nb", SourceSpan { start: 1, end: 3 }, (1, 2, 2, 1))]
#[case("é\n", SourceSpan { start: 0, end: 2 }, (1, 1, 1, 3))]
fn locations_are_one_based_byte_positions(
    #[case] source: &str,
    #[case] span: SourceSpan,
    #[case] expected: (usize, usize, usize, usize),
) {
    let location = LocationIndex::new(source)
        .locate(span)
        .expect("test span should be valid");
    assert_eq!(
        (
            location.start_line,
            location.start_column,
            location.end_line,
            location.end_column,
        ),
        expected
    );
}

#[rstest]
fn application_builds_ordered_facts() {
    let source = include_bytes!("fixtures/makefiles/all-facts.mk");
    let report =
        parse_source(source, "Makefile", &MakefileLosslessParser).expect("fixture should parse");

    assert_eq!(report.parse.status, ParseStatus::Complete);
    assert_eq!(report.variables.first().map(|fact| fact.ordinal), Some(0));
    assert_eq!(report.includes.first().map(|fact| fact.ordinal), Some(1));
    assert_eq!(
        report
            .rules
            .iter()
            .map(|fact| fact.ordinal)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(
        report.rules.first().map(|fact| fact.double_colon),
        Some(true)
    );
    let recipe = report.rules.first().and_then(|fact| fact.recipes.first());
    assert_eq!(recipe.map(|fact| fact.silent), Some(true));
    assert_eq!(recipe.map(|fact| fact.ignore_errors), Some(true));
    assert_eq!(recipe.map(|fact| fact.always_execute), Some(true));
}

#[rstest]
fn recovered_parse_retains_facts_and_diagnostics() {
    let source = include_bytes!("fixtures/makefiles/recovered.mk");
    let report = parse_source(source, "broken.mk", &MakefileLosslessParser)
        .expect("recovered parse should produce a report");

    assert_eq!(report.parse.status, ParseStatus::Recovered);
    assert!(!report.parse.diagnostics.is_empty());
    assert_eq!(
        report.variables.first().map(|fact| fact.name.as_str()),
        Some("GOOD")
    );
    assert!(
        report
            .rules
            .iter()
            .any(|fact| fact.targets.iter().any(|target| target == "valid"))
    );
}

#[rstest]
#[case("A = one\n", "=", "one")]
#[case("A := two\n", ":=", "two")]
#[case("A ::= three\n", "::=", "three")]
#[case("A :::= four\n", ":::=", "four")]
#[case("A += five\n", "+=", "five")]
#[case("A ?= six\n", "?=", "six")]
fn assignment_operators_remain_source_faithful(
    #[case] source: &str,
    #[case] operator: &str,
    #[case] raw_value: &str,
) {
    let report = parse_source(source.as_bytes(), "Makefile", &MakefileLosslessParser)
        .expect("assignment should parse");
    let variable = report
        .variables
        .first()
        .expect("one variable should be reported");
    assert_eq!(variable.operator, operator);
    assert_eq!(variable.raw_value, raw_value);
}

#[rstest]
fn shell_assignment_parser_gap_remains_explicit() {
    let report = parse_source(b"A != printf seven\n", "Makefile", &MakefileLosslessParser)
        .expect("upstream recovery should still produce a report");

    assert_eq!(report.parse.status, ParseStatus::Recovered);
    assert!(report.variables.is_empty());
    assert!(!report.parse.diagnostics.is_empty());
}

#[rstest]
fn nested_conditions_preserve_outer_to_inner_branches() {
    let source = b"ifdef OUTER\nifeq ($(A),yes)\nX = one\nelse\nX = two\nendif\nendif\n";
    let report = parse_source(source, "Makefile", &MakefileLosslessParser)
        .expect("nested conditionals should parse");
    let branches = report
        .variables
        .iter()
        .map(|variable| {
            variable
                .conditions
                .iter()
                .map(|condition| condition.branch)
                .collect()
        })
        .collect::<Vec<Vec<_>>>();
    assert_eq!(
        branches,
        [
            vec![
                makeutil::domain::ConditionBranch::If,
                makeutil::domain::ConditionBranch::If
            ],
            vec![
                makeutil::domain::ConditionBranch::If,
                makeutil::domain::ConditionBranch::Else
            ],
        ]
    );
}

proptest! {
    #[test]
    fn valid_ascii_spans_are_monotonic(
        source in "[ -~\\n\\r]{0,128}",
        first in 0usize..129,
        second in 0usize..129,
    ) {
        let start = first.min(second).min(source.len());
        let end = first.max(second).min(source.len());
        let location = LocationIndex::new(&source)
            .locate(SourceSpan { start, end })
            .expect("ASCII offsets are UTF-8 boundaries");
        prop_assert!(location.start_byte <= location.end_byte);
        prop_assert!(location.start_line <= location.end_line);
        prop_assert!(location.start_column >= 1);
        prop_assert!(location.end_column >= 1);
        prop_assert_eq!(source.get(start..end).map(str::len), Some(end - start));
    }
}
