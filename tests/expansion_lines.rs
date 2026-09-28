//! Report-shape regressions for bare expansion lines.
//!
//! GNU Make 4.4.1 expands a top-level line such as `$(info ...)` or
//! `$(eval ...)` and parses the result. `info`, `warning` and `error` expand
//! to empty text, so a line of those calls defines nothing and the report
//! stays `complete`. `eval`, `call`, `foreach` or a bare `$(VAR)` may define a
//! rule or a variable that a static parse cannot see, so each such line is a
//! diagnostic on its own line and the report stays `recovered`. Before the
//! parser learned these lines, each one became a rule missing its colon, and
//! its diagnostics landed on other lines.

use makeutil::{
    adapters::MakefileLosslessParser,
    domain::{ParseReport, ParseStatus},
    parse_source,
};
use pretty_assertions::assert_eq;
use rstest::rstest;

/// The one diagnostic message a bare expansion line produces.
const EXPANSION_MESSAGE: &str =
    "expansion line may define rules or variables a static parse cannot see";

/// Parse a fixture into a report.
fn report(source: &[u8], path: &str) -> Result<ParseReport, Box<dyn std::error::Error>> {
    Ok(parse_source(source, path, &MakefileLosslessParser)?)
}

/// Returns every rule target in the report, in order.
fn targets(report: &ParseReport) -> Vec<&str> {
    report
        .rules
        .iter()
        .flat_map(|rule| rule.targets.iter().map(String::as_str))
        .collect()
}

/// Returns each diagnostic as its message, line, and first and last column.
fn diagnostics(report: &ParseReport) -> Vec<(&str, usize, usize, usize)> {
    report
        .parse
        .diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.message.as_str(),
                diagnostic.location.start_line,
                diagnostic.location.start_column,
                diagnostic.location.end_column,
            )
        })
        .collect()
}

/// Lines of calls that expand to empty text add no fact and no diagnostic,
/// whether bare, commented, or guarded by a conditional.
#[rstest]
#[case::info_warning_error(
    include_bytes!("fixtures/makefiles/bare-info-expansion.mk"),
    "bare-info-expansion.mk",
    &["build"],
    &["NAME"],
)]
#[case::guarded_error(
    include_bytes!("fixtures/makefiles/conditional-error-directive.mk"),
    "conditional-error-directive.mk",
    &["build"],
    &["VERSION"],
)]
fn empty_expansions_parse_complete(
    #[case] source: &[u8],
    #[case] path: &str,
    #[case] expected_targets: &[&str],
    #[case] expected_variables: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let report = report(source, path)?;

    assert_eq!(report.parse.status, ParseStatus::Complete);
    assert_eq!(diagnostics(&report), []);
    assert_eq!(targets(&report), expected_targets);
    let variables: Vec<&str> = report
        .variables
        .iter()
        .map(|fact| fact.name.as_str())
        .collect();
    assert_eq!(variables, expected_variables);
    Ok(())
}

/// A whole project Makefile with two `$(error ...)` guards inside `ifneq`
/// blocks, beside `override` and `$(file <...)` assignments. The guards once
/// became two extra rules, 24 in all, and a `recovered` report.
#[rstest]
fn version_guarded_project_parses_complete_with_every_rule()
-> Result<(), Box<dyn std::error::Error>> {
    let report = report(
        include_bytes!("fixtures/makefiles/version-guarded-project.mk"),
        "version-guarded-project.mk",
    )?;

    assert_eq!(report.parse.status, ParseStatus::Complete);
    assert_eq!(diagnostics(&report), []);
    assert_eq!(report.rules.len(), 22);
    report
        .variables
        .iter()
        .find(|fact| fact.name == "KANI_VERSION")
        .ok_or("the `override` assignment beside the guards must be kept")?;
    Ok(())
}

/// Lines that may define structure are each reported on their own line and
/// span, and the facts around them survive.
#[rstest]
#[case::eval_and_foreach(
    include_bytes!("fixtures/makefiles/bare-eval-expansion.mk"),
    "bare-eval-expansion.mk",
    &[(9, 1, 23), (10, 1, 59)],
)]
#[case::bare_variable(
    include_bytes!("fixtures/makefiles/bare-variable-expansion.mk"),
    "bare-variable-expansion.mk",
    &[(5, 1, 18)],
)]
fn structural_expansions_recover_on_their_own_lines(
    #[case] source: &[u8],
    #[case] path: &str,
    #[case] expected_lines: &[(usize, usize, usize)],
) -> Result<(), Box<dyn std::error::Error>> {
    let report = report(source, path)?;

    assert_eq!(report.parse.status, ParseStatus::Recovered);
    let expected: Vec<_> = expected_lines
        .iter()
        .map(|&(line, start, end)| (EXPANSION_MESSAGE, line, start, end))
        .collect();
    assert_eq!(diagnostics(&report), expected);
    assert_eq!(
        targets(&report),
        ["build"],
        "no rule is invented for an expansion"
    );
    Ok(())
}

/// Lines that look like empty expansions but are not: literal text beside an
/// `info` call expands to that text, and `$(info)` with no argument is a
/// reference to a variable called `info`, whose value may be anything.
#[rstest]
#[case::literal_text_beside_a_call(b"$(info a) stray\n", 16)]
#[case::info_without_arguments(b"$(info)\n", 8)]
fn lookalike_lines_recover(
    #[case] source: &[u8],
    #[case] end_column: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = report(source, "inline.mk")?;

    assert_eq!(report.parse.status, ParseStatus::Recovered);
    assert_eq!(
        diagnostics(&report),
        [(EXPANSION_MESSAGE, 1, 1, end_column)]
    );
    assert_eq!(targets(&report), Vec::<&str>::new());
    Ok(())
}
