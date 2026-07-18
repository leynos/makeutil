//! Pure domain and application contract tests.

use makeutil::{
    adapters::MakefileLosslessParser,
    domain::{AssignmentOperator, ConditionKind, LocationIndex, ParseStatus, SourceSpan},
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
    assert_eq!(report.variables.len(), 4);
    assert_eq!(
        report
            .variables
            .iter()
            .find(|fact| fact.name == "RELEASE")
            .map(|fact| fact.exported),
        Some(true)
    );
    assert_eq!(
        report
            .variables
            .iter()
            .find(|fact| fact.name == "TOOL")
            .map(|fact| fact.overridden),
        Some(true)
    );
    assert_eq!(
        report
            .variables
            .iter()
            .find(|fact| fact.name == "SCRIPT")
            .map(|fact| fact.define_block),
        Some(true)
    );
    assert_eq!(report.includes.first().map(|fact| fact.ordinal), Some(4));
    assert_eq!(
        report
            .rules
            .iter()
            .map(|fact| fact.ordinal)
            .collect::<Vec<_>>(),
        [5, 6]
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
#[case("A = one\n", AssignmentOperator::Recursive, "one")]
#[case("A := two\n", AssignmentOperator::Simple, "two")]
#[case("A ::= three\n", AssignmentOperator::PosixSimple, "three")]
#[case("A :::= four\n", AssignmentOperator::ImmediateRecursive, "four")]
#[case("A += five\n", AssignmentOperator::Append, "five")]
#[case("A ?= six\n", AssignmentOperator::Conditional, "six")]
#[case("A != printf seven\n", AssignmentOperator::Shell, "printf seven")]
#[case::define_block(
    include_str!("fixtures/makefiles/multiline-define.mk"),
    AssignmentOperator::Define,
    "echo one  \necho two\t \n"
)]
fn assignment_operators_remain_source_faithful(
    #[case] source: &str,
    #[case] operator: AssignmentOperator,
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
#[case(AssignmentOperator::Define, r#""""#)]
#[case(AssignmentOperator::Recursive, r#""=""#)]
#[case(AssignmentOperator::Simple, r#"":=""#)]
#[case(AssignmentOperator::PosixSimple, r#""::=""#)]
#[case(AssignmentOperator::ImmediateRecursive, r#"":::=""#)]
#[case(AssignmentOperator::Append, r#""+=""#)]
#[case(AssignmentOperator::Conditional, r#""?=""#)]
#[case(AssignmentOperator::Shell, r#""!=""#)]
fn assignment_operators_match_schema_values(
    #[case] operator: AssignmentOperator,
    #[case] expected_json: &str,
) -> Result<(), serde_json::Error> {
    assert_eq!(serde_json::to_string(&operator)?, expected_json);
    Ok(())
}

#[rstest]
#[case("A = value  \n", "value  ")]
#[case("A = value\t \n", "value\t ")]
fn variable_values_preserve_trailing_whitespace(#[case] source: &str, #[case] raw_value: &str) {
    let report = parse_source(source.as_bytes(), "Makefile", &MakefileLosslessParser)
        .expect("assignment should parse");
    let variable = report
        .variables
        .first()
        .expect("one variable should be reported");
    assert_eq!(variable.raw_value, raw_value);
}

#[rstest]
#[case("@-+echo ok", true, true, true)]
#[case("@+-echo ok", true, true, true)]
#[case("-@+echo ok", true, true, true)]
#[case("-+@echo ok", true, true, true)]
#[case("+@-echo ok", true, true, true)]
#[case("+-@echo ok", true, true, true)]
#[case("echo + later", false, false, false)]
fn recipe_modifier_order_is_semantic(
    #[case] recipe: &str,
    #[case] silent: bool,
    #[case] ignore_errors: bool,
    #[case] always_execute: bool,
) {
    let source = format!("all:\n\t{recipe}\n");
    let report = parse_source(source.as_bytes(), "Makefile", &MakefileLosslessParser)
        .expect("recipe should parse");
    let parsed_recipe = report
        .rules
        .first()
        .and_then(|rule| rule.recipes.first())
        .expect("one recipe should be reported");
    assert_eq!(parsed_recipe.silent, silent);
    assert_eq!(parsed_recipe.ignore_errors, ignore_errors);
    assert_eq!(parsed_recipe.always_execute, always_execute);
}

#[rstest]
#[case("ifdef FLAG\nX = one\nendif\n", ConditionKind::Ifdef, "ifdef")]
#[case("ifndef FLAG\nX = one\nendif\n", ConditionKind::Ifndef, "ifndef")]
#[case("ifeq ($(A),yes)\nX = one\nendif\n", ConditionKind::Ifeq, "ifeq")]
#[case("ifneq ($(A),yes)\nX = one\nendif\n", ConditionKind::Ifneq, "ifneq")]
fn condition_kinds_use_the_closed_domain_type(
    #[case] source: &str,
    #[case] kind: ConditionKind,
    #[case] serialized_kind: &str,
) {
    let report = parse_source(source.as_bytes(), "Makefile", &MakefileLosslessParser)
        .expect("conditional should parse");
    let condition = report
        .variables
        .first()
        .and_then(|variable| variable.conditions.first())
        .expect("one condition should be reported");
    assert_eq!(condition.kind, kind);
    assert_eq!(
        serde_json::to_value(kind).expect("condition kind should serialize"),
        serialized_kind
    );
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
