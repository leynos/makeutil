//! Unit tests for operator mapping and `export`/`unexport` directive name
//! collection.

use makefile_lossless::{Makefile, MakefileItem, Parse, VariableDefinition};
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{OperatorContext, assignment_operator, directive_names, export_directive_observations};
use crate::{
    domain::{AssignmentOperator, ConditionBranch, ConditionKind, SourceSpan},
    ports::{ConditionObservation, ParserPortError, SyntaxObservation},
};

/// Span standing in for the directive's own range, which these tests do not
/// exercise.
const SPAN: SourceSpan = SourceSpan { start: 0, end: 0 };

/// Build a context from the modifiers a definition line carries.
fn context(is_define: bool, is_export: bool) -> OperatorContext {
    OperatorContext {
        is_define,
        is_export,
        is_unexport: false,
    }
}

/// Build the context a real definition line produces, as the adapter does.
fn context_of(variable: &VariableDefinition) -> OperatorContext {
    OperatorContext {
        is_define: variable.is_define(),
        is_export: variable.is_export(),
        is_unexport: variable.is_unexport(),
    }
}

/// Parse one source line and return its variable definition node, if upstream
/// modelled the line as one.
fn first_variable(source: &str) -> Option<VariableDefinition> {
    Parse::<Makefile>::parse_makefile(source)
        .tree()
        .items()
        .find_map(|item| match item {
            MakefileItem::Variable(variable) => Some(variable),
            _ => None,
        })
}

#[rstest]
fn define_without_operator_uses_empty_schema_variant() {
    assert_eq!(
        assignment_operator(None, context(true, false)),
        Ok(AssignmentOperator::Define)
    );
}

/// The directive path supplies the operator itself rather than routing through
/// [`assignment_operator`], so the empty-operator guarantee is pinned on the
/// facts a real directive produces.
#[rstest]
fn bare_export_directive_uses_empty_schema_variant() {
    let variable = first_variable("export MOLD_VERSION_FILE\n")
        .expect("a bare export should be modelled as a variable definition");
    let observations = export_directive_observations(&variable, context_of(&variable), &[], SPAN);

    assert_eq!(
        observations,
        vec![SyntaxObservation::Variable {
            name: "MOLD_VERSION_FILE".to_owned(),
            operator: AssignmentOperator::Define,
            raw_value: String::new(),
            exported: true,
            overridden: false,
            define_block: false,
            conditions: Vec::new(),
            span: SPAN,
        }]
    );
}

/// An `unexport` directive yields the same valueless fact, marked as not
/// exported, which is what tells it from an `export` directive.
#[rstest]
fn bare_unexport_directive_is_not_exported() {
    let variable = first_variable("unexport RUSTDOC_FLAGS\n")
        .expect("a bare unexport should be modelled as a variable definition");
    let observations = export_directive_observations(&variable, context_of(&variable), &[], SPAN);

    assert_eq!(
        observations,
        vec![SyntaxObservation::Variable {
            name: "RUSTDOC_FLAGS".to_owned(),
            operator: AssignmentOperator::Define,
            raw_value: String::new(),
            exported: false,
            overridden: false,
            define_block: false,
            conditions: Vec::new(),
            span: SPAN,
        }]
    );
}

/// `override export FOO` is a directive too, and the override modifier must
/// survive the expansion into per-name facts.
#[rstest]
fn overridden_export_directive_retains_its_modifier() {
    let variable = first_variable("override export FOO\n")
        .expect("an overridden export should be modelled as a variable definition");
    let observations = export_directive_observations(&variable, context_of(&variable), &[], SPAN);
    let overridden = observations.iter().map(|observation| match observation {
        SyntaxObservation::Variable {
            name, overridden, ..
        } => (name.as_str(), *overridden),
        _ => ("", false),
    });

    assert_eq!(overridden.collect::<Vec<_>>(), vec![("FOO", true)]);
}

/// The conditional ancestry of the directive line is carried onto every fact
/// it produces.
#[rstest]
fn export_directive_facts_carry_their_conditions() {
    let variable = first_variable("export FOO\n")
        .expect("a bare export should be modelled as a variable definition");
    let ancestry = vec![ConditionObservation {
        kind: ConditionKind::Ifdef,
        expression: "COND".to_owned(),
        branch: ConditionBranch::If,
        span: SPAN,
    }];
    let observations =
        export_directive_observations(&variable, context_of(&variable), &ancestry, SPAN);

    assert_eq!(
        observations
            .first()
            .and_then(|observation| match observation {
                SyntaxObservation::Variable { conditions, .. } => Some(conditions.clone()),
                _ => None,
            }),
        Some(ancestry)
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

/// A leading `unexport` makes a line a directive and decides that its facts
/// are not exported, even when `export` also appears on the line as a name.
///
/// Each expectation is `(is_directive_only, is_exported)`.
#[rstest]
#[case::unexport(unexport_line(false, false), (true, false))]
#[case::unexport_define(unexport_line(true, false), (false, false))]
#[case::unexport_naming_export(unexport_line(false, true), (true, false))]
#[case::export(context(false, true), (true, true))]
fn unexport_lines_are_directives_that_do_not_export(
    #[case] line: OperatorContext,
    #[case] expected: (bool, bool),
) {
    assert_eq!((line.is_directive_only(), line.is_exported()), expected);
}

/// Build the context of a line leading with `unexport`.
fn unexport_line(is_define: bool, is_export: bool) -> OperatorContext {
    OperatorContext {
        is_unexport: true,
        ..context(is_define, is_export)
    }
}

/// Every name a directive line carries is collected, now that the pinned
/// parser revision keeps the whole list inside the variable node.
///
/// The keyword-named cases guard the anchoring rule: a variable may be called
/// `unexport`, and `export export FOO` has upstream consume both leading
/// keywords, so neither may be decided by matching keyword text.
#[rstest]
#[case::single("export FOO\n", vec!["FOO"])]
#[case::multiple("export FOO BAR BAZ\n", vec!["FOO", "BAR", "BAZ"])]
#[case::continued("export FOO \\\n\tBAR\n", vec!["FOO", "BAR"])]
#[case::name_less("export\n", Vec::new())]
#[case::keyword_named_variable("export unexport\n", vec!["unexport"])]
#[case::repeated_keyword_prefix("export export FOO\n", vec!["FOO"])]
#[case::overridden("override export FOO\n", vec!["FOO"])]
#[case::unnameable("export export\n", Vec::new())]
#[case::unexport_single("unexport FOO\n", vec!["FOO"])]
#[case::unexport_multiple("unexport FOO BAR\n", vec!["FOO", "BAR"])]
#[case::unexport_name_less("unexport\n", Vec::new())]
#[case::unexport_keyword_named_variable("unexport unexport\n", vec!["unexport"])]
fn bare_export_names_are_all_collected(#[case] source: &str, #[case] expected: Vec<&str>) {
    let variable = first_variable(source)
        .expect("an export directive should be modelled as a variable definition");

    assert_eq!(directive_names(&variable), expected);
}
