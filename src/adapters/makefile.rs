//! `makefile-lossless` 0.3.40 adapter for the domain-owned parser port.

use makefile_lossless::{Conditional, Makefile, MakefileItem, Parse, SyntaxKind};
use rowan::ast::AstNode as _;

use crate::{
    domain::{ConditionBranch, SourceSpan},
    ports::{
        ConditionObservation,
        MakefileParser,
        ParserOutcome,
        ParserPortError,
        RecipeObservation,
        SyntaxObservation,
    },
};

/// GNU Make parser backed by the exactly pinned lossless CST crate.
#[derive(Debug, Clone, Copy, Default)]
pub struct MakefileLosslessParser;

impl MakefileParser for MakefileLosslessParser {
    fn parse(&self, source: &str) -> Result<ParserOutcome, ParserPortError> {
        let parsed = Parse::<Makefile>::parse_makefile(source);
        let tree = parsed.tree();
        if tree.to_string() != source {
            return Err(ParserPortError::RoundTripMismatch);
        }

        let mut observations = Vec::new();
        collect_items(tree.items(), &[], source.len(), &mut observations)?;
        collect_diagnostics(&parsed, source, &mut observations)?;
        Ok(ParserOutcome { observations })
    }
}

fn collect_items(
    items: impl Iterator<Item = MakefileItem>,
    conditions: &[ConditionObservation],
    source_length: usize,
    observations: &mut Vec<SyntaxObservation>,
) -> Result<(), ParserPortError> {
    for item in items {
        match item {
            MakefileItem::Rule(rule) => {
                let recipes = rule
                    .recipe_nodes()
                    .map(|recipe| {
                        let text = recipe.text();
                        Ok(RecipeObservation {
                            silent: recipe.is_silent(),
                            ignore_errors: recipe.is_ignore_errors(),
                            always_execute: text.trim_start_matches(['@', '-']).starts_with('+')
                                || text.starts_with('+'),
                            text,
                            span: span(recipe.text_range(), source_length)?,
                        })
                    })
                    .collect::<Result<Vec<_>, ParserPortError>>()?;
                observations.push(SyntaxObservation::Rule {
                    targets: rule.targets().collect(),
                    prerequisites: rule.prerequisites().collect(),
                    double_colon: rule.is_double_colon(),
                    conditions: conditions.to_vec(),
                    recipes,
                    span: span(rule.syntax().text_range(), source_length)?,
                });
            }
            MakefileItem::Variable(variable) => {
                observations.push(SyntaxObservation::Variable {
                    name: variable.name().ok_or(ParserPortError::MissingField {
                        field: "variable-name",
                    })?,
                    operator: variable.assignment_operator().unwrap_or_default(),
                    raw_value: variable.raw_value().unwrap_or_default().trim().to_owned(),
                    exported: variable.is_export(),
                    overridden: variable.is_override(),
                    define_block: variable.is_define(),
                    conditions: conditions.to_vec(),
                    span: span(variable.syntax().text_range(), source_length)?,
                });
            }
            MakefileItem::Include(include) => {
                observations.push(SyntaxObservation::Include {
                    raw_path: include.path().ok_or(ParserPortError::MissingField {
                        field: "include-path",
                    })?,
                    optional: include.is_optional(),
                    conditions: conditions.to_vec(),
                    span: span(include.syntax().text_range(), source_length)?,
                });
            }
            MakefileItem::Conditional(conditional) => {
                collect_conditional(&conditional, conditions, source_length, observations)?;
            }
            MakefileItem::Vpath(_) => {}
        }
    }
    Ok(())
}

fn collect_conditional(
    conditional: &Conditional,
    outer: &[ConditionObservation],
    source_length: usize,
    observations: &mut Vec<SyntaxObservation>,
) -> Result<(), ParserPortError> {
    let opening = conditional
        .syntax()
        .children()
        .find(|node| node.kind() == SyntaxKind::CONDITIONAL_IF)
        .ok_or(ParserPortError::MissingField {
            field: "conditional-opening",
        })?;
    let kind = conditional
        .conditional_type()
        .ok_or(ParserPortError::MissingField {
            field: "conditional-kind",
        })?;
    let expression = conditional.condition().unwrap_or_default();
    let mut if_conditions = outer.to_vec();
    if_conditions.push(ConditionObservation {
        kind: kind.clone(),
        expression: expression.clone(),
        branch: ConditionBranch::If,
        span: span(opening.text_range(), source_length)?,
    });
    collect_items(
        conditional.if_items(),
        &if_conditions,
        source_length,
        observations,
    )?;

    collect_else_branch(
        conditional,
        outer,
        ElseBranch {
            kind,
            expression,
            source_length,
        },
        observations,
    )?;
    Ok(())
}

fn collect_else_branch(
    conditional: &Conditional,
    outer: &[ConditionObservation],
    branch: ElseBranch,
    observations: &mut Vec<SyntaxObservation>,
) -> Result<(), ParserPortError> {
    if !conditional.has_else() {
        return Ok(());
    }
    let else_node = conditional
        .syntax()
        .children()
        .find(|node| node.kind() == SyntaxKind::CONDITIONAL_ELSE)
        .ok_or(ParserPortError::MissingField {
            field: "conditional-else",
        })?;
    let mut else_conditions = outer.to_vec();
    else_conditions.push(ConditionObservation {
        kind: branch.kind,
        expression: branch.expression,
        branch: ConditionBranch::Else,
        span: span(else_node.text_range(), branch.source_length)?,
    });
    collect_items(
        conditional.else_items(),
        &else_conditions,
        branch.source_length,
        observations,
    )
}

struct ElseBranch {
    kind: String,
    expression: String,
    source_length: usize,
}

fn collect_diagnostics(
    parsed: &Parse<Makefile>,
    source: &str,
    observations: &mut Vec<SyntaxObservation>,
) -> Result<(), ParserPortError> {
    if !parsed.positioned_errors().is_empty() {
        for error in parsed.positioned_errors() {
            observations.push(SyntaxObservation::Diagnostic {
                message: error.message.clone(),
                code: error.code.clone(),
                span: span(error.range, source.len())?,
            });
        }
        return Ok(());
    }
    for error in parsed.errors() {
        observations.push(SyntaxObservation::Diagnostic {
            message: error.message.clone(),
            code: None,
            span: line_span(source, error.line),
        });
    }
    Ok(())
}

fn line_span(source: &str, one_based_line: usize) -> SourceSpan {
    let target = one_based_line.saturating_sub(1);
    let mut start = 0_usize;
    let mut end = source.len();
    for (line, segment) in source.split_inclusive('\n').enumerate() {
        if line == target {
            end = start.saturating_add(segment.trim_end_matches(['\r', '\n']).len());
            break;
        }
        start = start.saturating_add(segment.len());
    }
    SourceSpan { start, end }
}

fn span(
    range: makefile_lossless::TextRange,
    source_length: usize,
) -> Result<SourceSpan, ParserPortError> {
    SourceSpan::new(range.start().into(), range.end().into(), source_length).map_err(Into::into)
}
