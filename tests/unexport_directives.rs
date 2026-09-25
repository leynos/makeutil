//! Report-shape regressions for GNU Make's `unexport` directive.
//!
//! `unexport NAME` keeps a variable out of recipe environments. Schema version
//! 1 represents it as `export NAME` is represented, by a valueless fact with
//! the empty operator and `define_block` false, and `exported` false tells the
//! two directives apart (ADR-0002). Before the parser learned the keyword, an
//! `unexport` line was read as a rule missing its colon, forced a `recovered`
//! report, and placed both of its diagnostics on the wrong lines.

use makeutil::{
    adapters::MakefileLosslessParser,
    domain::{AssignmentOperator, ParseReport, ParseStatus, VariableFact},
    parse_source,
};
use pretty_assertions::assert_eq;
use rstest::rstest;

/// Operator, raw value, export flag and `define_block` of one variable fact.
type FactShape<'a> = (AssignmentOperator, &'a str, bool, bool);

/// Summarize the fields that identify a fact as a directive or an assignment.
const fn shape(fact: &VariableFact) -> FactShape<'_> {
    (
        fact.operator,
        fact.raw_value.as_str(),
        fact.exported,
        fact.define_block,
    )
}

/// Find the fact for `name` whose operator satisfies `is_wanted`.
fn fact<'report>(
    report: &'report ParseReport,
    name: &str,
    is_wanted: impl Fn(AssignmentOperator) -> bool,
) -> Result<&'report VariableFact, String> {
    report
        .variables
        .iter()
        .find(|fact| fact.name == name && is_wanted(fact.operator))
        .ok_or_else(|| format!("the report must contain a fact for {name}"))
}

/// Parse `source` and require a complete report with no diagnostics.
fn complete_report(source: &[u8], path: &str) -> Result<ParseReport, Box<dyn std::error::Error>> {
    let report = parse_source(source, path, &MakefileLosslessParser)?;
    assert_eq!(
        (report.parse.status, report.parse.diagnostics.as_slice()),
        (ParseStatus::Complete, [].as_slice()),
        "{path} must parse complete",
    );
    Ok(report)
}

/// The shape that exposed the gap: an `unexport` after a conditional
/// assignment, followed by an exported assignment and a rule. Every fact is
/// kept, and the directive is the only one with `exported` false.
#[rstest]
fn unexport_after_a_conditional_assignment_is_complete() -> Result<(), Box<dyn std::error::Error>> {
    let report = complete_report(
        include_bytes!("fixtures/makefiles/unexport-after-conditional.mk"),
        "unexport-after-conditional.mk",
    )?;

    let directive = fact(&report, "DOC_FLAGS", |operator| {
        operator == AssignmentOperator::Define
    })?;
    let assignment = fact(&report, "DOC_FLAGS", |operator| {
        operator == AssignmentOperator::Conditional
    })?;
    let exported = fact(&report, "DOCFLAGS", |_| true)?;
    assert_eq!(
        (shape(directive), shape(assignment), shape(exported)),
        (
            (AssignmentOperator::Define, "", false, false),
            (
                AssignmentOperator::Conditional,
                "--cfg docsrs -D warnings",
                false,
                false
            ),
            (
                AssignmentOperator::Simple,
                "$(value DOC_FLAGS)",
                true,
                false
            ),
        ),
    );
    assert_eq!(
        report
            .rules
            .iter()
            .map(|rule| rule.targets.clone())
            .collect::<Vec<_>>(),
        vec![vec!["docs".to_owned()]],
        "no rule may be invented for the unexport line",
    );
    Ok(())
}

/// Both reproductions of the misplaced diagnostics now parse complete. The
/// first reported lines 1 and 5 for an `unexport` on line 4; the second, with
/// a rule appended, reported lines 2 and 8.
#[rstest]
#[case::directive_last("A = 1\nB = 2\nC = 3\nunexport C\n")]
#[case::rule_after("A = 1\nB = 2\nC = 3\nunexport C\n\nall:\n\techo done\n")]
fn unexport_no_longer_misplaces_diagnostics(
    #[case] source: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = complete_report(source.as_bytes(), "unexport.mk")?;
    let directive = fact(&report, "C", |operator| {
        operator == AssignmentOperator::Define
    })?;

    assert_eq!(
        (shape(directive), directive.location.start_line),
        ((AssignmentOperator::Define, "", false, false), 4)
    );
    Ok(())
}

/// `unexport` takes the same shapes as `export` in GNU Make 4.4.1, and every
/// fact it yields has `exported` false.
#[rstest]
#[case::several_names("unexport FOO BAR\n", vec![("FOO", AssignmentOperator::Define, ""), ("BAR", AssignmentOperator::Define, "")])]
#[case::assignment("unexport FOO = 3\n", vec![("FOO", AssignmentOperator::Recursive, "3")])]
#[case::overridden_name("unexport override FOO = 3\n", vec![("FOO", AssignmentOperator::Recursive, "3")])]
fn unexport_facts_are_not_exported(
    #[case] source: &str,
    #[case] expected: Vec<(&str, AssignmentOperator, &str)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = complete_report(source.as_bytes(), "unexport.mk")?;
    let facts: Vec<_> = report
        .variables
        .iter()
        .map(|fact| {
            (
                (fact.name.as_str(), fact.operator, fact.raw_value.as_str()),
                fact.exported,
            )
        })
        .collect();

    assert_eq!(
        facts,
        expected
            .into_iter()
            .map(|summary| (summary, false))
            .collect::<Vec<_>>()
    );
    Ok(())
}

/// The keyword is a directive only when it leads the line; after `export` it
/// is the name of the exported variable.
#[rstest]
fn a_variable_named_unexport_is_still_exported() -> Result<(), Box<dyn std::error::Error>> {
    let report = complete_report(b"export unexport\n", "export.mk")?;
    let exported = fact(&report, "unexport", |_| true)?;

    assert_eq!(
        shape(exported),
        (AssignmentOperator::Define, "", true, false)
    );
    Ok(())
}

/// `unexport override FOO` unexports a variable called `override` as well as
/// `FOO`, but the parser never names `override`. The nameable fact survives,
/// not exported, and a diagnostic keeps the report from claiming `complete`.
#[rstest]
fn an_unnameable_unexported_name_is_diagnosed() -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(
        b"unexport override FOO\n",
        "unexport.mk",
        &MakefileLosslessParser,
    )?;
    let names: Vec<_> = report
        .variables
        .iter()
        .map(|fact| (fact.name.as_str(), fact.exported))
        .collect();
    let messages: Vec<_> = report
        .parse
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();

    assert_eq!(
        (report.parse.status, names, messages),
        (
            ParseStatus::Recovered,
            vec![("FOO", false)],
            vec!["some unexport directive names could not be represented"]
        )
    );
    Ok(())
}

/// On an assignment, `export` is a modifier wherever it appears: GNU Make
/// 4.4.1 assigns and exports on `unexport export FOO = 3`, so the fact is
/// exported. On a directive line the leading `unexport` decides instead.
#[rstest]
#[case::assignment("unexport export FOO = 3\n", AssignmentOperator::Recursive, true)]
#[case::directive("unexport export FOO\n", AssignmentOperator::Define, false)]
fn export_after_unexport_follows_gnu_make(
    #[case] source: &str,
    #[case] operator: AssignmentOperator,
    #[case] exported: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(source.as_bytes(), "unexport.mk", &MakefileLosslessParser)?;
    let foo = fact(&report, "FOO", |_| true)?;

    assert_eq!((foo.operator, foo.exported), (operator, exported));
    Ok(())
}
