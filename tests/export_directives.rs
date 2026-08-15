//! Report-shape regressions for GNU Make's `export` directive family.
//!
//! A bare `export NAME` names a variable without assigning a value on that
//! line, so it carries no assignment operator. Schema version 1 has no
//! dedicated representation for such a directive, so these tests pin the
//! chosen one — an entry in `variables` with the empty operator — and pin the
//! never-abort guarantee for every form the parser cannot represent faithfully.

use makefile_lossless::{Makefile, Parse};
use makeutil::{
    ParseApplicationError,
    adapters::MakefileLosslessParser,
    domain::{AssignmentOperator, ParseReport, ParseStatus, SourceLocation, VariableFact},
    parse_source,
};
use pretty_assertions::assert_eq;
use proptest::{prelude::*, test_runner::TestCaseError};
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

/// No `export` or `unexport` form may abort the parse, and none may go missing
/// from a report that still claims `complete`.
///
/// The variable names are asserted alongside the status so that a form which
/// silently stopped producing its fact could not keep passing. A form the
/// parser cannot name yields no fact, and the adapter's own diagnostic — not
/// upstream's, which is not always present — keeps such a report `recovered`.
#[rstest]
#[case::single("export FOO\n", ParseStatus::Complete, vec!["FOO"], None)]
#[case::multiple("export FOO BAR BAZ\n", ParseStatus::Complete, vec!["FOO", "BAR", "BAZ"], None)]
#[case::continued("export FOO \\\n\tBAR\n", ParseStatus::Complete, vec!["FOO", "BAR"], None)]
#[case::name_less("export\n", ParseStatus::Recovered, Vec::new(), Some((0, 7)))]
#[case::keyword_named_variable("export unexport\n", ParseStatus::Complete, vec!["unexport"], None)]
// Upstream cannot represent the first exported name when it matches one of
// its directive keywords. Retain every later name it can identify, but mark
// the omitted name with a located recovery diagnostic.
#[case::repeated_keyword_prefix("export export FOO\n", ParseStatus::Recovered, vec!["FOO"], Some((0, 18)))]
#[case::override_named_variable("export override FOO\n", ParseStatus::Recovered, vec!["FOO"], Some((0, 20)))]
#[case::unnameable("export export\n", ParseStatus::Recovered, Vec::new(), Some((0, 14)))]
// GNU Make actually rejects `override export FOO` with "missing separator".
// The parser accepts the single-name form, which predates this work and is
// left alone; only the multi-name form was in scope. Pinned as it behaves.
#[case::overridden("override export FOO\n", ParseStatus::Complete, vec!["FOO"], None)]
#[case::overridden_list("override export FOO BAR\n", ParseStatus::Recovered, vec!["FOO"], None)]
#[case::undiagnosed_upstream(
    "override export override\n",
    ParseStatus::Recovered,
    Vec::new(),
    None
)]
#[case::exported_define(
    "export define FOO\nbody\nendef\n",
    ParseStatus::Recovered,
    Vec::new(),
    None
)]
#[case::name_less_exported_define(
    "export define\nbody\nendef\n",
    ParseStatus::Recovered,
    Vec::new(),
    None
)]
#[case::assignment("export FOO := bar\n", ParseStatus::Complete, vec!["FOO"], None)]
#[case::unexport_single("unexport FOO\n", ParseStatus::Recovered, Vec::new(), None)]
#[case::unexport_multiple("unexport FOO BAR\n", ParseStatus::Recovered, Vec::new(), None)]
#[case::unexport_name_less("unexport\n", ParseStatus::Recovered, Vec::new(), None)]
fn no_export_form_aborts(
    #[case] source: &str,
    #[case] expected: ParseStatus,
    #[case] expected_names: Vec<&str>,
    #[case] expected_diagnostic_span: Option<(usize, usize)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(source.as_bytes(), "export.mk", &MakefileLosslessParser)?;
    let names: Vec<&str> = report
        .variables
        .iter()
        .map(|variable| variable.name.as_str())
        .collect();

    assert_eq!((report.parse.status, names), (expected, expected_names));
    if expected == ParseStatus::Recovered && report.parse.diagnostics.is_empty() {
        return Err("a recovered report must explain itself with a diagnostic".into());
    }
    if let Some((start_byte, end_byte)) = expected_diagnostic_span {
        let expected_location = SourceLocation {
            start_byte,
            end_byte,
            start_line: 1,
            start_column: 1,
            end_line: 2,
            end_column: 1,
        };
        if !report
            .parse
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.location == expected_location)
        {
            return Err("the recovered diagnostic must cover the complete directive line".into());
        }
    }
    Ok(())
}

/// A definition the parser cannot name and that is not an `export` is a broken
/// tree, not an unrepresentable construct, so it must still fail loudly rather
/// than being quietly dropped along with the export forms.
#[rstest]
fn a_name_less_definition_that_is_not_an_export_still_fails() {
    let outcome = parse_source(b"define\n", "define.mk", &MakefileLosslessParser);

    assert_eq!(
        outcome.err().map(|error| error.to_string()),
        Some("required variable-name accessor was absent".to_owned())
    );
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

    assert_eq!(
        (report.parse.status, names),
        (ParseStatus::Recovered, Vec::new())
    );
    if report.parse.diagnostics.is_empty() {
        return Err("dropping the block must be explained by a diagnostic".into());
    }
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

/// The real-world shape: one directive exporting several already-assigned
/// variables, which must now report every name and a complete parse.
#[rstest]
fn multi_name_export_is_complete() -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(
        include_bytes!("fixtures/makefiles/export-directive-list.mk"),
        "export-directive-list.mk",
        &MakefileLosslessParser,
    )?;

    assert_eq!(report.parse.status, ParseStatus::Complete);
    assert_eq!(report.parse.diagnostics, Vec::new());

    let expected_span = (120, 186);
    for name in [
        "MOLD_VERSION_FILE",
        "MOLD_SHA256SUMS_FILE",
        "RUST_TOOLCHAIN_FILE",
    ] {
        let fact = directive(&report, name)
            .ok_or_else(|| format!("the report must contain a directive fact for {name}"))?;
        assert_eq!(
            (
                fact.exported,
                fact.overridden,
                fact.define_block,
                fact.raw_value.as_str()
            ),
            (true, false, false, ""),
            "{name} must be an exported, valueless directive",
        );
        assert_eq!(
            (fact.location.start_byte, fact.location.end_byte),
            expected_span,
            "{name} must use the directive line's full span",
        );
    }
    Ok(())
}

proptest! {
    /// Every valid name on a bare export becomes one ordered directive fact.
    #[test]
    fn multi_name_bare_export_preserves_facts_and_source(
        names in proptest::collection::vec("[A-Z][A-Z0-9_]{0,12}", 1..12),
    ) {
        let source = format!("export {}\n", names.join(" "));
        let parsed = Parse::<Makefile>::parse_makefile(&source);
        prop_assert_eq!(parsed.tree().to_string(), source.as_str());

        let report = parse_source(
            source.as_bytes(),
            "generated-export.mk",
            &MakefileLosslessParser,
        )
        .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let facts = report
            .variables
            .iter()
            .filter(|variable| variable.operator == AssignmentOperator::Define)
            .collect::<Vec<_>>();
        let observed_names = facts
            .iter()
            .map(|variable| variable.name.clone())
            .collect::<Vec<_>>();
        let expected_span = (0, source.len());

        prop_assert_eq!(report.parse.status, ParseStatus::Complete);
        prop_assert!(report.parse.diagnostics.is_empty());
        prop_assert_eq!(facts.len(), names.len());
        prop_assert_eq!(observed_names.as_slice(), names.as_slice());
        for fact in facts {
            prop_assert_eq!(fact.raw_value.as_str(), "");
            prop_assert!(fact.exported);
            prop_assert!(!fact.overridden);
            prop_assert!(!fact.define_block);
            prop_assert_eq!(
                (fact.location.start_byte, fact.location.end_byte),
                expected_span,
            );
        }
    }
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
