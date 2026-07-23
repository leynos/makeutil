//! Adapter invariant tests for unsupported upstream syntax.

use makefile_lossless::{Makefile, Parse};
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{
    MakefileLosslessParser,
    assignment_operator,
    collect_diagnostics,
    condition_kind,
    ensure_round_trip,
};
use crate::{
    domain::AssignmentOperator,
    ports::{MakefileParser as _, ParserPortError, SyntaxObservation},
};

#[rstest]
fn unknown_condition_kind_is_rejected() {
    assert_eq!(
        condition_kind("ifunknown"),
        Err(ParserPortError::UnsupportedConditionKind {
            kind: "ifunknown".to_owned(),
        })
    );
}

#[rstest]
fn round_trip_mismatch_is_rejected() {
    let parsed = Parse::<Makefile>::parse_makefile("all:\n");

    assert_eq!(
        ensure_round_trip(&parsed.tree(), "different:\n"),
        Err(ParserPortError::RoundTripMismatch)
    );
}

#[rstest]
fn define_without_operator_uses_empty_schema_variant() {
    assert_eq!(
        assignment_operator(None, true),
        Ok(AssignmentOperator::Define)
    );
}

#[rstest]
fn ordinary_variable_requires_an_operator() {
    assert_eq!(
        assignment_operator(None, false),
        Err(ParserPortError::MissingField {
            field: "variable-assignment-operator",
        })
    );
}

#[rstest]
fn unsupported_assignment_operator_is_rejected() {
    assert_eq!(
        assignment_operator(Some("unknown"), false),
        Err(ParserPortError::UnsupportedAssignmentOperator {
            operator: "unknown".to_owned(),
        })
    );
}

#[rstest]
fn multiline_define_preserves_exact_body() {
    let source = include_str!("../../tests/fixtures/makefiles/multiline-define.mk");
    let outcome = MakefileLosslessParser
        .parse(source)
        .expect("multiline define fixture should parse");
    let variable = outcome.observations.iter().find_map(|observation| {
        if let SyntaxObservation::Variable {
            operator,
            raw_value,
            ..
        } = observation
        {
            Some((operator.to_owned(), raw_value.as_str()))
        } else {
            None
        }
    });

    assert_eq!(
        variable,
        Some((AssignmentOperator::Define, "echo one  \necho two\t \n"))
    );
}

#[rstest]
fn all_upstream_diagnostic_channels_are_retained_for_large_sources() {
    let source = "broken rule without colon\n".repeat(4_096);
    let parsed = Parse::<Makefile>::parse_makefile(&source);
    assert!(!parsed.positioned_errors().is_empty());
    assert!(!parsed.errors().is_empty());

    let mut observations = Vec::new();
    collect_diagnostics(&parsed, &source, &mut observations)
        .expect("valid upstream diagnostic spans should be retained");

    assert_eq!(
        observations.len(),
        parsed.positioned_errors().len() + parsed.errors().len()
    );
}
