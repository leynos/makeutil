//! `makefile-lossless` 0.3.40 adapter for the domain-owned parser port.

use std::collections::BTreeMap;

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

use super::makefile_export::{OperatorContext, assignment_operator, export_directive_observations};
use crate::{
    domain::{ConditionBranch, ConditionKind, SourceSpan},
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
        collect_items(tree.items(), source.len(), &mut observations)?;
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
    source_length: usize,
    observations: &mut Vec<SyntaxObservation>,
) -> Result<(), ParserPortError> {
    let mut pending = items
        .map(TraversalEvent::Item)
        .collect::<Vec<TraversalEvent>>();
    pending.reverse();
    let mut conditions = Vec::new();

    while let Some(event) = pending.pop() {
        match event {
            TraversalEvent::Push(condition) => conditions.push(condition),
            TraversalEvent::Restore(depth) => conditions.truncate(depth),
            TraversalEvent::Item(MakefileItem::Rule(rule)) => {
                observations.push(rule_observation(&rule, &conditions, source_length)?);
            }
            TraversalEvent::Item(MakefileItem::Variable(variable)) => {
                observations.extend(variable_observation(&variable, &conditions, source_length)?);
            }
            TraversalEvent::Item(MakefileItem::Include(include)) => {
                observations.push(include_observation(&include, &conditions, source_length)?);
            }
            TraversalEvent::Item(MakefileItem::Conditional(conditional)) => {
                schedule_conditional(&conditional, source_length, conditions.len(), &mut pending)?;
            }
            TraversalEvent::Item(MakefileItem::Vpath(_)) => {}
        }
    }
    Ok(())
}

enum TraversalEvent {
    Item(MakefileItem),
    Push(ConditionObservation),
    Restore(usize),
}

fn schedule_conditional(
    conditional: &Conditional,
    source_length: usize,
    outer_depth: usize,
    pending: &mut Vec<TraversalEvent>,
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
    let if_condition = ConditionObservation {
        kind,
        expression: expression.clone(),
        branch: ConditionBranch::If,
        span: span(opening.text_range(), source_length)?,
    };
    let mut events = vec![TraversalEvent::Push(if_condition)];
    events.extend(conditional.if_items().map(TraversalEvent::Item));
    events.push(TraversalEvent::Restore(outer_depth));

    if conditional.has_else() {
        let else_node = conditional
            .syntax()
            .children()
            .find(|node| node.kind() == SyntaxKind::CONDITIONAL_ELSE)
            .ok_or(ParserPortError::MissingField {
                field: "conditional-else",
            })?;
        events.push(TraversalEvent::Push(ConditionObservation {
            kind,
            expression,
            branch: ConditionBranch::Else,
            span: span(else_node.text_range(), source_length)?,
        }));
        events.extend(conditional.else_items().map(TraversalEvent::Item));
        events.push(TraversalEvent::Restore(outer_depth));
    }

    pending.extend(events.into_iter().rev());
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

/// One `export A B C` line names several variables, and a name-less `export`
/// names none, so a single definition node can yield any number of facts.
fn variable_observation(
    variable: &VariableDefinition,
    conditions: &[ConditionObservation],
    source_length: usize,
) -> Result<Vec<SyntaxObservation>, ParserPortError> {
    let operator = variable.assignment_operator();
    let context = OperatorContext {
        is_define: variable.is_define(),
        is_export: variable.is_export(),
    };
    let definition_span = span(variable.syntax().text_range(), source_length)?;

    if operator.is_none() && context.is_directive_only() {
        return Ok(export_directive_observations(
            variable,
            conditions,
            definition_span,
        ));
    }

    Ok(vec![SyntaxObservation::Variable {
        name: variable.name().ok_or(ParserPortError::MissingField {
            field: "variable-name",
        })?,
        operator: assignment_operator(operator.as_deref(), context)?,
        raw_value: variable.raw_value().unwrap_or_default(),
        exported: context.is_export,
        overridden: variable.is_override(),
        define_block: context.is_define,
        conditions: conditions.to_vec(),
        span: definition_span,
    }])
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
    let mut line_spans: BTreeMap<_, Option<SourceSpan>> = parsed
        .errors()
        .iter()
        .map(|error| (error.line.saturating_sub(1), None))
        .collect();
    let mut unresolved_lines = line_spans.len();
    let mut start = 0_usize;
    for (line, segment) in source.split_inclusive('\n').enumerate() {
        if let Some(resolved_span) = line_spans.get_mut(&line) {
            *resolved_span = Some(SourceSpan {
                start,
                end: start.saturating_add(segment.trim_end_matches(['\r', '\n']).len()),
            });
            unresolved_lines = unresolved_lines.saturating_sub(1);
        }
        start = start.saturating_add(segment.len());
        if unresolved_lines == 0 {
            break;
        }
    }
    let end_of_source = SourceSpan {
        start: source.len(),
        end: source.len(),
    };
    for error in parsed.errors() {
        observations.push(SyntaxObservation::Diagnostic {
            message: error.message.clone(),
            code: None,
            span: line_spans
                .get(&error.line.saturating_sub(1))
                .copied()
                .flatten()
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
