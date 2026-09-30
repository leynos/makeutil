//! Bare expansion lines for the parser adapter.
//!
//! GNU Make expands a top-level line such as `$(info ...)` or `$(eval ...)`
//! and parses the result. A static parse cannot expand, so this module decides
//! which of these lines the report can see through. GNU Make 4.4.1 expands
//! `info`, `warning` and `error` to empty text, so a line holding only those
//! calls defines nothing and needs no fact. Any other bare expansion, such as
//! `$(eval ...)`, `$(call ...)` or a bare `$(VAR)`, may define a rule or a
//! variable the report cannot show. It becomes a diagnostic on its own line,
//! which keeps the parse `recovered` rather than overclaiming `complete`.

use makefile_lossless::{Expansion, VariableReference};
use rowan::ast::AstNode as _;

use crate::{
    domain::SourceSpan,
    ports::{ParserPortError, SyntaxObservation},
};

/// Functions that GNU Make expands to empty text, so a call defines nothing.
const EMPTY_EXPANSION_FUNCTIONS: [&str; 3] = ["info", "warning", "error"];

/// Functions whose expansion runs Make text the tree cannot show: `eval`
/// parses its result as Makefile syntax, and `call` runs a variable that may
/// hold an `eval`. Nested in an argument, either makes the outer call unsafe.
const OPAQUE_FUNCTIONS: [&str; 2] = ["eval", "call"];

/// Returns the diagnostic for a bare expansion line that may define structure,
/// or `None` when the line holds only calls that expand to empty text.
///
/// # Errors
///
/// Returns [`ParserPortError`] when the line's range lies outside the source.
pub(super) fn expansion_observation(
    expansion: &Expansion,
    source: &str,
) -> Result<Option<SyntaxObservation>, ParserPortError> {
    if expands_to_nothing(expansion) {
        return Ok(None);
    }
    Ok(Some(SyntaxObservation::Diagnostic {
        message: "expansion line may define rules or variables a static parse cannot see"
            .to_owned(),
        code: None,
        span: line_span(expansion, source)?,
    }))
}

/// Returns true if every reference on the line is a call to a function that
/// expands to empty text, with nothing else on the line.
fn expands_to_nothing(expansion: &Expansion) -> bool {
    expansion.has_only_references()
        && expansion
            .references()
            .all(|reference| is_empty_expansion_call(&reference))
}

/// Returns true for a call to `info`, `warning` or `error` whose arguments
/// hold no `eval` or `call`, however deeply nested.
///
/// A call needs whitespace after the function name. The parser also reports
/// `$(info,foo)` as a call, but GNU Make reads that as the variable
/// `info,foo`, whose value may be anything, so it is not an empty expansion.
///
/// Make expands the arguments before printing them, so
/// `$(info $(eval X := 1))` still defines `X`. A plain variable reference in an
/// argument is accepted: its value is opaque to a static parse, but so is every
/// variable the report names, and flagging each one would recover nearly every
/// real diagnostic message.
fn is_empty_expansion_call(reference: &VariableReference) -> bool {
    reference.is_function_call()
        && reference
            .name()
            .is_some_and(|name| EMPTY_EXPANSION_FUNCTIONS.contains(&name.as_str()))
        && has_function_separator(reference)
        && !has_opaque_argument(reference)
}

/// Returns true when whitespace follows the name inside the reference, as a
/// function call requires.
fn has_function_separator(reference: &VariableReference) -> bool {
    let text = reference.syntax().text().to_string();
    let inner = text.get(2..).unwrap_or_default();
    let after_name =
        inner.trim_start_matches(|c: char| !c.is_whitespace() && !matches!(c, ',' | ')' | '}'));
    after_name.starts_with(char::is_whitespace)
}

/// Returns true if any reference nested inside `reference` is an `eval` or
/// `call`.
fn has_opaque_argument(reference: &VariableReference) -> bool {
    reference
        .syntax()
        .descendants()
        .skip(1)
        .filter_map(VariableReference::cast)
        .filter(VariableReference::is_function_call)
        .filter_map(|nested| nested.name())
        .any(|name| OPAQUE_FUNCTIONS.contains(&name.as_str()))
}

/// Returns the span of the line without its trailing newline, so the
/// diagnostic names the expansion's own line.
fn line_span(expansion: &Expansion, source: &str) -> Result<SourceSpan, ParserPortError> {
    let range = expansion.syntax().text_range();
    let start = usize::from(range.start());
    let text = source
        .get(start..usize::from(range.end()))
        .unwrap_or_default();
    let end = start.saturating_add(text.trim_end_matches(['\r', '\n']).len());
    SourceSpan::new(start, end, source.len()).map_err(Into::into)
}
