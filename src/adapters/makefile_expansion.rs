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

/// Returns true for a call to `info`, `warning` or `error`.
fn is_empty_expansion_call(reference: &VariableReference) -> bool {
    reference.is_function_call()
        && reference
            .name()
            .is_some_and(|name| EMPTY_EXPANSION_FUNCTIONS.contains(&name.as_str()))
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
