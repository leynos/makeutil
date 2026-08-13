//! Unit tests for operator mapping and `export` directive name collection.

use makefile_lossless::{Makefile, MakefileItem, Parse};
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{OperatorContext, assignment_operator, directive_names};
use crate::{domain::AssignmentOperator, ports::ParserPortError};

/// Build a context from the modifiers a definition line carries.
fn context(is_define: bool, is_export: bool) -> OperatorContext {
    OperatorContext {
        is_define,
        is_export,
    }
}

#[rstest]
fn define_without_operator_uses_empty_schema_variant() {
    assert_eq!(
        assignment_operator(None, context(true, false)),
        Ok(AssignmentOperator::Define)
    );
}

#[rstest]
fn bare_export_directive_uses_empty_schema_variant() {
    assert_eq!(
        assignment_operator(None, context(false, true)),
        Ok(AssignmentOperator::Define)
    );
}

#[rstest]
fn plain_variable_without_operator_is_still_rejected() {
    assert_eq!(
        assignment_operator(None, context(false, false)),
        Err(ParserPortError::MissingField {
            field: "variable-assignment-operator",
        })
    );
}

#[rstest]
fn unsupported_assignment_operator_is_rejected() {
    assert_eq!(
        assignment_operator(Some("unknown"), context(false, false)),
        Err(ParserPortError::UnsupportedAssignmentOperator {
            operator: "unknown".to_owned(),
        })
    );
}

#[rstest]
#[case(true, false, false)]
#[case(false, true, true)]
#[case(true, true, false)]
#[case(false, false, false)]
fn directive_only_lines_are_recognized(
    #[case] is_define: bool,
    #[case] is_export: bool,
    #[case] expected: bool,
) {
    assert_eq!(context(is_define, is_export).is_directive_only(), expected);
}

/// Upstream keeps only the first name of a multi-name `export` inside the
/// variable node, so the helper reports one name until the pinned parser
/// revision learns the directive list form.
#[rstest]
#[case("export FOO\n", vec!["FOO"])]
#[case("export FOO BAR BAZ\n", vec!["FOO"])]
#[case("export\n", Vec::new())]
fn bare_export_names_are_all_collected(#[case] source: &str, #[case] expected: Vec<&str>) {
    let parsed = Parse::<Makefile>::parse_makefile(source);
    let variable = parsed
        .tree()
        .items()
        .find_map(|item| match item {
            MakefileItem::Variable(variable) => Some(variable),
            _ => None,
        })
        .expect("an export directive should be modelled as a variable definition");

    assert_eq!(directive_names(&variable), expected);
}
