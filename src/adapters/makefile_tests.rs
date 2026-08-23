//! Adapter invariant tests for unsupported upstream syntax.

use std::fmt::Write as _;

use makefile_lossless::{Makefile, Parse};
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{MakefileLosslessParser, collect_diagnostics, condition_kind, ensure_round_trip};
use crate::{
    domain::{AssignmentOperator, ConditionBranch, ConditionKind, SourceSpan},
    ports::{ConditionObservation, MakefileParser as _, ParserPortError, SyntaxObservation},
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
fn deeply_nested_conditionals_use_iterative_ancestry() {
    const DEPTH: usize = 256;

    let mut source = String::new();
    let mut opening_spans = Vec::with_capacity(DEPTH);
    for depth in 0..DEPTH {
        let start = source.len();
        writeln!(&mut source, "ifdef LEVEL_{depth}")
            .expect("writing a generated Makefile to a String should succeed");
        opening_spans.push(SourceSpan {
            start,
            end: source.len(),
        });
    }
    source.push_str("VALUE = yes\n");
    source.push_str(&"endif\n".repeat(DEPTH));

    let outcome = MakefileLosslessParser
        .parse(&source)
        .expect("256 nested conditionals should parse without recursive traversal");
    let conditions = outcome
        .observations
        .iter()
        .find_map(|observation| {
            if let SyntaxObservation::Variable { conditions, .. } = observation {
                Some(conditions)
            } else {
                None
            }
        })
        .expect("the generated variable should be observed");
    let first_span = opening_spans
        .first()
        .copied()
        .expect("the generated source should contain an opening directive");
    let last_span = opening_spans
        .last()
        .copied()
        .expect("the generated source should contain an opening directive");

    assert_eq!(conditions.len(), DEPTH);
    assert_eq!(
        conditions.first(),
        Some(&ConditionObservation {
            kind: ConditionKind::Ifdef,
            expression: "LEVEL_0".to_owned(),
            branch: ConditionBranch::If,
            span: first_span,
        })
    );
    assert_eq!(
        conditions.last(),
        Some(&ConditionObservation {
            kind: ConditionKind::Ifdef,
            expression: "LEVEL_255".to_owned(),
            branch: ConditionBranch::If,
            span: last_span,
        })
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

    let positioned_count = parsed.positioned_errors().len();
    assert_eq!(observations.len(), positioned_count + parsed.errors().len());
    assert_eq!(
        observations.first(),
        Some(&SyntaxObservation::Diagnostic {
            message: "expected ':'".to_owned(),
            code: None,
            span: SourceSpan {
                start: 106_470,
                end: 106_476,
            },
        })
    );
    assert_eq!(
        observations.get(1),
        Some(&SyntaxObservation::Diagnostic {
            message: "expected ':'".to_owned(),
            code: None,
            span: SourceSpan {
                start: 106_444,
                end: 106_450,
            },
        })
    );
    assert_eq!(
        observations.get(positioned_count.saturating_sub(1)),
        Some(&SyntaxObservation::Diagnostic {
            message: "expected ':'".to_owned(),
            code: None,
            span: SourceSpan { start: 0, end: 6 },
        })
    );
    assert_eq!(
        observations.get(positioned_count),
        Some(&SyntaxObservation::Diagnostic {
            message: "expected ':'".to_owned(),
            code: None,
            span: SourceSpan {
                start: source.len(),
                end: source.len(),
            },
        })
    );
}
