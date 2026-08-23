//! Unsupported-syntax corpus regressions.
//!
//! Each fixture is a reduced unsupported-syntax regression fixture. The tests
//! pin the parser's honest behaviour for constructs it cannot yet represent:
//! the parse must degrade to `recovered` with a positioned diagnostic, never
//! report a false `complete`. If an upstream `makefile-lossless` release learns
//! one of these constructs, the corresponding test fails on purpose so the pin
//! and the expectations are revisited together.

use makeutil::{adapters::MakefileLosslessParser, domain::ParseStatus, parse_source};
use pretty_assertions::assert_eq;
use rstest::rstest;

/// An `unexport` directive is not recognized as a directive at all: upstream
/// parses it as a rule whose first target is the word `unexport`. Schema
/// version 1 cannot express "this name was explicitly un-exported", so the
/// misleading rule is pinned here rather than corrected. If a future upstream
/// release learns the directive, this test fails and forces the pin and the
/// representation to be revisited together.
#[rstest]
fn unexport_directive_degrades_honestly() {
    let report = parse_source(
        include_bytes!("fixtures/makefiles/export-directive-limits.mk"),
        "export-directive-limits.mk",
        &MakefileLosslessParser,
    )
    .expect("the export-directive-limits corpus fixture must parse into a report");

    assert_eq!(report.parse.status, ParseStatus::Recovered);
    assert!(
        !report.parse.diagnostics.is_empty(),
        "a recovered parse must carry at least one diagnostic",
    );
    assert!(
        report
            .rules
            .iter()
            .any(|rule| rule.targets.first().map(String::as_str) == Some("unexport")),
        "the misleading unexport rule must remain pinned: {:?}",
        report
            .rules
            .iter()
            .map(|rule| rule.targets.clone())
            .collect::<Vec<_>>(),
    );
}

/// A bare `$(error ...)` directive inside a conditional must parse as
/// recovered, with the
/// surrounding facts retained.
#[rstest]
fn bare_error_directive_recovers_with_facts_retained() {
    let report = parse_source(
        include_bytes!("fixtures/makefiles/conditional-error-directive.mk"),
        "conditional-error-directive.mk",
        &MakefileLosslessParser,
    )
    .expect("the conditional-error-directive corpus fixture must parse into a report");

    assert_eq!(report.parse.status, ParseStatus::Recovered);
    assert!(
        !report.parse.diagnostics.is_empty(),
        "a recovered parse must carry at least one diagnostic",
    );
    assert!(
        report.parse.diagnostics.iter().any(|diagnostic| {
            diagnostic.location.start_byte == 330 && diagnostic.location.end_byte == 331
        }),
        "the recovered parse must retain the positioned upstream diagnostic",
    );

    let variable_names: Vec<&str> = report
        .variables
        .iter()
        .map(|variable| variable.name.as_str())
        .collect();
    assert!(
        variable_names.contains(&"VERSION"),
        "facts before the unsupported directive must survive: {variable_names:?}",
    );
    let rule_targets: Vec<&str> = report
        .rules
        .iter()
        .flat_map(|rule| rule.targets.iter().map(String::as_str))
        .collect();
    assert!(
        rule_targets.contains(&"build"),
        "facts after the unsupported directive must survive: {rule_targets:?}",
    );
}
