//! Report-shape regressions for GNU Make's `export` directive family.
//!
//! A bare `export NAME` names a variable assigned elsewhere, so it carries no
//! assignment operator. Schema version 1 has no dedicated representation for
//! such a directive, so these tests pin the chosen one — an entry in
//! `variables` with the empty operator — and pin the never-abort guarantee for
//! every form the parser cannot represent faithfully.

use makeutil::{
    ParseApplicationError,
    adapters::MakefileLosslessParser,
    domain::{AssignmentOperator, ParseReport, ParseStatus, VariableFact},
    parse_source,
};
use pretty_assertions::assert_eq;
use rstest::{fixture, rstest};

#[fixture]
fn bare_export_report() -> Result<ParseReport, ParseApplicationError> {
    parse_source(
        include_bytes!("fixtures/makefiles/bare-export.mk"),
        "bare-export.mk",
        &MakefileLosslessParser,
    )
}

fn variable<'report>(report: &'report ParseReport, name: &str) -> Option<&'report VariableFact> {
    report
        .variables
        .iter()
        .find(|variable| variable.name == name)
}

/// Pins the consumer-facing discriminating predicate for bare export
/// directives: an empty `operator` with `define_block` false identifies a
/// directive rather than an assignment, and `raw_value` is empty.
#[rstest]
fn single_name_bare_export_is_complete(
    bare_export_report: Result<ParseReport, ParseApplicationError>,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = bare_export_report?;

    assert_eq!(report.parse.status, ParseStatus::Complete);
    assert_eq!(report.parse.diagnostics, Vec::new());

    for name in ["MOLD_VERSION_FILE", "RUST_TOOLCHAIN_FILE"] {
        let directive = report
            .variables
            .iter()
            .find(|variable| {
                variable.name == name && variable.operator == AssignmentOperator::Define
            })
            .ok_or_else(|| format!("the report must contain a directive fact for {name}"))?;
        assert_eq!(
            (
                directive.exported,
                directive.define_block,
                directive.raw_value.as_str()
            ),
            (true, false, ""),
            "{name} must be an exported, valueless directive rather than a define block",
        );
    }
    Ok(())
}

#[rstest]
fn assignments_and_directives_are_distinguishable(
    bare_export_report: Result<ParseReport, ParseApplicationError>,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = bare_export_report?;
    let assignment = variable(&report, "CARGO_TERM_COLOR")
        .ok_or("the report must contain the CARGO_TERM_COLOR assignment")?;

    assert_eq!(
        (
            assignment.operator,
            assignment.exported,
            assignment.raw_value.as_str()
        ),
        (AssignmentOperator::Simple, true, "always"),
    );
    Ok(())
}

#[rstest]
fn facts_after_a_bare_export_survive(
    bare_export_report: Result<ParseReport, ParseApplicationError>,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = bare_export_report?;
    let targets: Vec<&str> = report
        .rules
        .iter()
        .flat_map(|rule| rule.targets.iter().map(String::as_str))
        .collect();

    if targets.contains(&"build") {
        Ok(())
    } else {
        Err(format!("facts after a bare export must survive: {targets:?}").into())
    }
}

#[rstest]
fn multi_name_export_never_aborts() -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(
        include_bytes!("fixtures/makefiles/export-directive-list.mk"),
        "export-directive-list.mk",
        &MakefileLosslessParser,
    )?;

    assert_eq!(report.parse.status, ParseStatus::Recovered);
    Ok(())
}

#[rstest]
fn name_less_export_degrades_to_a_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(
        include_bytes!("fixtures/makefiles/export-directive-limits.mk"),
        "export-directive-limits.mk",
        &MakefileLosslessParser,
    )?;

    assert_eq!(report.parse.status, ParseStatus::Recovered);
    if report.parse.diagnostics.is_empty() {
        return Err("a recovered parse must carry at least one diagnostic".into());
    }
    let names: Vec<&str> = report
        .variables
        .iter()
        .map(|variable| variable.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["FOO"],
        "no variable fact may be invented for a name-less export",
    );
    Ok(())
}
