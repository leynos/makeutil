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

/// Find the directive fact for a name.
///
/// A name may appear twice — once for its assignment and once for the `export`
/// directive that names it — so the empty operator, not the name, selects the
/// directive.
fn directive<'report>(report: &'report ParseReport, name: &str) -> Option<&'report VariableFact> {
    report
        .variables
        .iter()
        .find(|variable| variable.name == name && variable.operator == AssignmentOperator::Define)
}

/// No `export` or `unexport` form may abort the parse.
///
/// A form upstream cannot name is dropped rather than invented, so the status
/// must be `recovered`: reporting `complete` while discarding a construct
/// would tell a consumer the facts are trustworthy when they are incomplete.
#[rstest]
#[case::single("export FOO\n", ParseStatus::Complete)]
#[case::multiple("export FOO BAR BAZ\n", ParseStatus::Recovered)]
#[case::name_less("export\n", ParseStatus::Recovered)]
#[case::keyword_named_variable("export unexport\n", ParseStatus::Complete)]
#[case::repeated_keyword_prefix("export export FOO\n", ParseStatus::Complete)]
#[case::unnameable("export export\n", ParseStatus::Recovered)]
#[case::overridden("override export FOO\n", ParseStatus::Complete)]
#[case::exported_define("export define FOO\nbody\nendef\n", ParseStatus::Recovered)]
#[case::name_less_exported_define("export define\nbody\nendef\n", ParseStatus::Recovered)]
#[case::assignment("export FOO := bar\n", ParseStatus::Complete)]
#[case::unexport_single("unexport FOO\n", ParseStatus::Recovered)]
#[case::unexport_multiple("unexport FOO BAR\n", ParseStatus::Recovered)]
#[case::unexport_name_less("unexport\n", ParseStatus::Recovered)]
fn no_export_form_aborts(
    #[case] source: &str,
    #[case] expected: ParseStatus,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(source.as_bytes(), "export.mk", &MakefileLosslessParser)?;

    assert_eq!(report.parse.status, expected);
    Ok(())
}

/// A variable may legitimately be called `unexport`. Deciding directive names
/// by keyword text would discard it while still reporting `complete`.
#[rstest]
fn a_variable_named_like_a_keyword_is_not_discarded() -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(b"export unexport\n", "export.mk", &MakefileLosslessParser)?;
    let directive =
        variable(&report, "unexport").ok_or("the exported name must not be discarded")?;

    assert_eq!(
        (
            directive.operator,
            directive.exported,
            directive.define_block
        ),
        (AssignmentOperator::Define, true, false),
    );
    Ok(())
}

/// An `export define` block is dropped rather than invented, so no fact may
/// claim the keyword itself was exported.
#[rstest]
fn exported_define_block_invents_no_fact() -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(
        b"export define FOO\nbody\nendef\n",
        "export.mk",
        &MakefileLosslessParser,
    )?;
    let names: Vec<&str> = report
        .variables
        .iter()
        .map(|variable| variable.name.as_str())
        .collect();

    assert_eq!(names, Vec::<&str>::new());
    Ok(())
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
        let directive = directive(&report, name)
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
