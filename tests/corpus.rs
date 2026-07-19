//! Real-estate corpus regressions.
//!
//! Each fixture here reduces a construct observed in a leynos repository
//! during the Operation Parabellum estate audit. The tests pin the parser's
//! honest behaviour for constructs it cannot yet represent: the parse must
//! degrade to `recovered` with a positioned diagnostic, never report a
//! false `complete`. If an upstream `makefile-lossless` release learns one
//! of these constructs, the corresponding test fails on purpose so the pin
//! and the expectations are revisited together.

use makeutil::{adapters::MakefileLosslessParser, domain::ParseStatus, parse_source};
use pretty_assertions::assert_eq;
use rstest::rstest;

/// A bare `$(error ...)` directive inside a conditional (from
/// leynos/pg-embed-setup-unpriv) must parse as recovered, with the
/// surrounding facts retained.
#[rstest]
fn bare_error_directive_recovers_with_facts_retained() -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(
        include_bytes!("fixtures/makefiles/conditional-error-directive.mk"),
        "conditional-error-directive.mk",
        &MakefileLosslessParser,
    )?;

    assert_eq!(report.parse.status, ParseStatus::Recovered);
    assert!(
        !report.parse.diagnostics.is_empty(),
        "a recovered parse must carry at least one diagnostic",
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
    Ok(())
}
