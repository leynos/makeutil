//! Assignment-operator and `export`/`unexport` directive handling for the
//! parser adapter.
//!
//! GNU Make's `export` and `unexport` keywords each serve two roles that look
//! alike in the tree: they can modify a definition (`export FOO := bar`) or
//! stand alone as a directive naming variables assigned elsewhere
//! (`unexport FOO BAR`). The directive form carries no assignment operator and
//! no value, so it needs separate treatment from an ordinary definition. This
//! module owns that distinction and the mapping of upstream operator tokens
//! onto the schema's operator set.

use makefile_lossless::{SyntaxKind, VariableDefinition};
use rowan::ast::AstNode as _;

use crate::{
    domain::{AssignmentOperator, SourceSpan},
    ports::{ConditionObservation, ParserPortError, SyntaxObservation},
};

/// Modifiers that make an absent assignment operator legitimate.
#[derive(Debug, Clone, Copy)]
pub(super) struct OperatorContext {
    /// Whether the definition is a `define` … `endef` block.
    pub(super) is_define: bool,
    /// Whether the `export` directive keyword is present.
    pub(super) is_export: bool,
    /// Whether the line leads with the `unexport` directive keyword.
    pub(super) is_unexport: bool,
}

impl OperatorContext {
    /// Whether the line only names variables assigned elsewhere, rather than
    /// carrying a definition of its own.
    pub(super) const fn is_directive_only(self) -> bool {
        self.names_a_directive() && !self.is_define
    }

    /// Whether the line carries the `export` or `unexport` directive keyword.
    pub(super) const fn names_a_directive(self) -> bool { self.is_export || self.is_unexport }

    /// Whether the facts a directive-only line yields are exported.
    ///
    /// A leading `unexport` decides, even when `export` appears later on the
    /// line as a name: GNU Make reads `unexport export FOO` as unexporting
    /// both. An assignment reads `is_export` instead, because there `export`
    /// is a modifier wherever it appears.
    pub(super) const fn is_exported(self) -> bool { self.is_export && !self.is_unexport }

    /// The directive keyword the line leads with, for diagnostics.
    pub(super) const fn directive_keyword(self) -> &'static str {
        if self.is_unexport {
            "unexport"
        } else {
            "export"
        }
    }
}

/// Map an upstream operator token onto the schema version 1 operator set.
///
/// An absent operator is legitimate for a `define` block and for a bare
/// `export` or `unexport` directive; all use the schema's empty operator. Any other
/// operator-less definition is a broken tree and must fail loudly.
///
/// The `is_export` half of that allowance is unreachable as the caller is
/// written: an operator-less export reaching here must also be a `define`,
/// which the first disjunct already covers. It is retained only so the
/// function is correct in isolation — an operator-less `export` has a
/// well-defined answer whatever routes to it — and never as a live guard.
pub(super) fn assignment_operator(
    operator: Option<&str>,
    context: OperatorContext,
) -> Result<AssignmentOperator, ParserPortError> {
    match operator {
        None if context.is_define || context.is_directive_only() => Ok(AssignmentOperator::Define),
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

/// Expand a directive-only `export` or `unexport` line into one valueless fact
/// per name, exported for `export` and not for `unexport`.
///
/// A name-less directive applies to every variable, which schema version 1
/// cannot express. `variable_observation` emits the recovery diagnostic before
/// this helper runs, so this helper only expands directives with a nameable
/// fact.
pub(super) fn export_directive_observations(
    variable: &VariableDefinition,
    context: OperatorContext,
    conditions: &[ConditionObservation],
    definition_span: SourceSpan,
) -> Vec<SyntaxObservation> {
    let mut observations = directive_names(variable)
        .into_iter()
        .map(|name| SyntaxObservation::Variable {
            name,
            operator: AssignmentOperator::Define,
            raw_value: String::new(),
            exported: context.is_exported(),
            overridden: variable.is_override(),
            define_block: false,
            conditions: conditions.to_vec(),
            span: definition_span,
        })
        .collect::<Vec<_>>();
    if has_unrepresentable_name_before_anchor(variable) {
        observations.push(SyntaxObservation::Diagnostic {
            message: format!(
                "some {} directive names could not be represented",
                context.directive_keyword()
            ),
            code: None,
            span: definition_span,
        });
    }
    observations
}

/// Whether a bare `export` or `unexport` starts with a name that upstream
/// omits before its first usable name.
///
/// Upstream treats `export`, `override`, and `define` as directive keywords
/// when choosing `VariableDefinition::name()`. On `export export FOO` and
/// `export override FOO`, the second identifier is actually an exported name
/// but is omitted before the `FOO` anchor. The fact cannot be represented in
/// schema version 1, so the caller must mark the otherwise usable facts as
/// recovered rather than silently reporting a complete parse.
fn has_unrepresentable_name_before_anchor(variable: &VariableDefinition) -> bool {
    let Some(first_name) = variable.name() else {
        return false;
    };
    let mut identifiers = variable
        .syntax()
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .filter(|token| token.kind() == SyntaxKind::IDENTIFIER)
        .map(|token| token.text().to_owned());
    if !matches!(identifiers.next().as_deref(), Some("export" | "unexport")) {
        return false;
    }
    identifiers
        .take_while(|name| name != &first_name)
        .next()
        .is_some()
}

/// Collect every identifier a directive-only `export` or `unexport` line
/// names.
///
/// A bare `export A B C` names three variables but upstream's
/// `VariableDefinition::name()` returns only the first, so the remaining names
/// have to be read from the node's own identifier tokens.
///
/// That first name anchors the walk rather than a list of keywords to skip:
/// the directive keywords upstream consumed are exactly the identifier tokens
/// preceding it, and a variable may legitimately be called `unexport`.
/// Filtering by keyword text instead would silently discard `export unexport`,
/// which upstream parses cleanly. Names upstream itself treats as keywords —
/// `export`, `override` and `define` — remain unrepresentable, because it
/// reports no name for them at all; the caller turns that into a diagnostic.
///
/// The first identifier of a directive line is always a keyword (`export`,
/// `unexport` or `override`), so it is skipped before anchoring. Otherwise
/// `unexport unexport`, whose name is the second token, would anchor on the
/// keyword and report the name twice.
///
/// The walk assumes the anchor is a direct identifier token of the definition
/// node, which is how upstream finds it too. Should that ever cease to hold,
/// the anchor is still reported on its own rather than the line being dropped.
pub(super) fn directive_names(variable: &VariableDefinition) -> Vec<String> {
    let Some(first_name) = variable.name() else {
        return Vec::new();
    };
    let names: Vec<String> = variable
        .syntax()
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .filter(|token| token.kind() == SyntaxKind::IDENTIFIER)
        .map(|token| token.text().to_owned())
        .skip(1)
        .skip_while(|text| *text != first_name)
        .collect();
    if names.is_empty() {
        vec![first_name]
    } else {
        names
    }
}

#[cfg(test)]
#[path = "makefile_export_tests.rs"]
mod tests;
