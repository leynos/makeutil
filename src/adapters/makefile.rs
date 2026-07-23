//! `makefile-lossless` 0.3.40 adapter for the domain-owned parser port.

use makefile_lossless::{
    Conditional,
    Include,
    Makefile,
    MakefileItem,
    Parse,
    Rule,
    SyntaxKind,
    VariableDefinition,
};
use rowan::ast::AstNode as _;

use crate::{
    domain::{AssignmentOperator, ConditionBranch, ConditionKind, SourceSpan},
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
        ensure_round_trip(&tree, source)?;

        let mut observations = Vec::new();
        collect_items(tree.items(), &[], source.len(), &mut observations)?;
        collect_diagnostics(&parsed, source, &mut observations)?;
        Ok(ParserOutcome { observations })
    }
}

fn ensure_round_trip(tree: &Makefile, source: &str) -> Result<(), ParserPortError> {
    if tree.to_string() == source {
        Ok(())
    } else {
        Err(ParserPortError::RoundTripMismatch)
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
                observations.push(rule_observation(&rule, conditions, source_length)?);
            }
            MakefileItem::Variable(variable) => {
                observations.push(variable_observation(&variable, conditions, source_length)?);
            }
            MakefileItem::Include(include) => {
                observations.push(include_observation(&include, conditions, source_length)?);
            }
            MakefileItem::Conditional(conditional) => {
                collect_conditional(&conditional, conditions, source_length, observations)?;
            }
            MakefileItem::Vpath(_) => {}
        }
    }
    Ok(())
}

fn rule_observation(
    rule: &Rule,
    conditions: &[ConditionObservation],
    source_length: usize,
) -> Result<SyntaxObservation, ParserPortError> {
    let recipes = rule
        .recipe_nodes()
        .map(|recipe| {
            let text = recipe.text();
            let modifiers = recipe_modifiers(&text);
            Ok(RecipeObservation {
                silent: modifiers.silent,
                ignore_errors: modifiers.ignore_errors,
                always_execute: modifiers.always_execute,
                text,
                span: span(recipe.text_range(), source_length)?,
            })
        })
        .collect::<Result<Vec<_>, ParserPortError>>()?;
    Ok(SyntaxObservation::Rule {
        targets: rule.targets().collect(),
        prerequisites: rule.prerequisites().collect(),
        double_colon: rule.is_double_colon(),
        conditions: conditions.to_vec(),
        recipes,
        span: span(rule.syntax().text_range(), source_length)?,
    })
}

fn variable_observation(
    variable: &VariableDefinition,
    conditions: &[ConditionObservation],
    source_length: usize,
) -> Result<SyntaxObservation, ParserPortError> {
    Ok(SyntaxObservation::Variable {
        name: variable.name().ok_or(ParserPortError::MissingField {
            field: "variable-name",
        })?,
        operator: assignment_operator(
            variable.assignment_operator().as_deref(),
            variable.is_define(),
        )?,
        raw_value: variable.raw_value().unwrap_or_default(),
        exported: variable.is_export(),
        overridden: variable.is_override(),
        define_block: variable.is_define(),
        conditions: conditions.to_vec(),
        span: span(variable.syntax().text_range(), source_length)?,
    })
}

fn assignment_operator(
    operator: Option<&str>,
    is_define: bool,
) -> Result<AssignmentOperator, ParserPortError> {
    match operator {
        None if is_define => Ok(AssignmentOperator::Define),
        Some("=") => Ok(AssignmentOperator::Recursive),
        Some(":=") => Ok(AssignmentOperator::Simple),
        Some("::=") => Ok(AssignmentOperator::PosixSimple),
        Some(":::=") => Ok(AssignmentOperator::ImmediateRecursive),
        Some("+=") => Ok(AssignmentOperator::Append),
        Some("?=") => Ok(AssignmentOperator::Conditional),
        Some("!=") => Ok(AssignmentOperator::Shell),
        Some(raw_operator) => Err(ParserPortError::UnsupportedAssignmentOperator {
            operator: raw_operator.to_owned(),
        }),
        None => Err(ParserPortError::MissingField {
            field: "variable-assignment-operator",
        }),
    }
}

fn include_observation(
    include: &Include,
    conditions: &[ConditionObservation],
    source_length: usize,
) -> Result<SyntaxObservation, ParserPortError> {
    Ok(SyntaxObservation::Include {
        raw_path: include.path().ok_or(ParserPortError::MissingField {
            field: "include-path",
        })?,
        optional: include.is_optional(),
        conditions: conditions.to_vec(),
        span: span(include.syntax().text_range(), source_length)?,
    })
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
    let raw_kind = conditional
        .conditional_type()
        .ok_or(ParserPortError::MissingField {
            field: "conditional-kind",
        })?;
    let kind = condition_kind(&raw_kind)?;
    let expression = conditional.condition().unwrap_or_default();
    let mut if_conditions = outer.to_vec();
    if_conditions.push(ConditionObservation {
        kind,
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
    kind: ConditionKind,
    expression: String,
    source_length: usize,
}

#[derive(Debug, Default)]
struct RecipeModifiers {
    silent: bool,
    ignore_errors: bool,
    always_execute: bool,
}

fn recipe_modifiers(text: &str) -> RecipeModifiers {
    text.chars()
        .take_while(|character| matches!(character, '@' | '-' | '+'))
        .fold(RecipeModifiers::default(), |mut modifiers, character| {
            match character {
                '@' => modifiers.silent = true,
                '-' => modifiers.ignore_errors = true,
                '+' => modifiers.always_execute = true,
                _ => {}
            }
            modifiers
        })
}

fn condition_kind(kind: &str) -> Result<ConditionKind, ParserPortError> {
    match kind {
        "ifdef" => Ok(ConditionKind::Ifdef),
        "ifndef" => Ok(ConditionKind::Ifndef),
        "ifeq" => Ok(ConditionKind::Ifeq),
        "ifneq" => Ok(ConditionKind::Ifneq),
        _ => Err(ParserPortError::UnsupportedConditionKind {
            kind: kind.to_owned(),
        }),
    }
}

fn collect_diagnostics(
    parsed: &Parse<Makefile>,
    source: &str,
    observations: &mut Vec<SyntaxObservation>,
) -> Result<(), ParserPortError> {
    for error in parsed.positioned_errors() {
        observations.push(SyntaxObservation::Diagnostic {
            message: error.message.clone(),
            code: error.code.clone(),
            span: span(error.range, source.len())?,
        });
    }
    let line_spans: Vec<_> = if parsed.errors().is_empty() {
        Vec::new()
    } else {
        source
            .split_inclusive('\n')
            .scan(0_usize, |start, segment| {
                let span = SourceSpan {
                    start: *start,
                    end: start.saturating_add(segment.trim_end_matches(['\r', '\n']).len()),
                };
                *start = start.saturating_add(segment.len());
                Some(span)
            })
            .collect()
    };
    let end_of_source = SourceSpan {
        start: source.len(),
        end: source.len(),
    };
    for error in parsed.errors() {
        observations.push(SyntaxObservation::Diagnostic {
            message: error.message.clone(),
            code: None,
            span: line_spans
                .get(error.line.saturating_sub(1))
                .copied()
                .unwrap_or(end_of_source),
        });
    }
    Ok(())
}

fn span(
    range: makefile_lossless::TextRange,
    source_length: usize,
) -> Result<SourceSpan, ParserPortError> {
    SourceSpan::new(range.start().into(), range.end().into(), source_length).map_err(Into::into)
}

#[cfg(test)]
#[path = "makefile_tests.rs"]
mod tests;
