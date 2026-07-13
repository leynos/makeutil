//! Domain-owned port isolating syntax collection from the upstream parser.

use thiserror::Error;

use crate::domain::{ConditionBranch, LocationError, SourceSpan};

/// Parser output expressed only in makeutil-owned observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserOutcome {
    /// Ordered facts and diagnostics.
    pub observations: Vec<SyntaxObservation>,
}

/// Conditional ancestry before display locations are calculated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionObservation {
    /// GNU Make conditional keyword.
    pub kind: String,
    /// Unexpanded expression.
    pub expression: String,
    /// Branch containing the fact.
    pub branch: ConditionBranch,
    /// Opening or else directive span.
    pub span: SourceSpan,
}

/// Recipe syntax associated with a rule observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeObservation {
    /// Recipe content without the tab or line ending.
    pub text: String,
    /// Whether `@` is present.
    pub silent: bool,
    /// Whether `-` is present.
    pub ignore_errors: bool,
    /// Whether `+` is present.
    pub always_execute: bool,
    /// Complete physical span.
    pub span: SourceSpan,
}

/// One ordered syntax fact or diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxObservation {
    /// Explicit rule syntax.
    Rule {
        /// Unexpanded targets.
        targets: Vec<String>,
        /// Unexpanded prerequisites.
        prerequisites: Vec<String>,
        /// Whether `::` is used.
        double_colon: bool,
        /// Conditional ancestry.
        conditions: Vec<ConditionObservation>,
        /// Ordered recipe syntax.
        recipes: Vec<RecipeObservation>,
        /// Complete rule span.
        span: SourceSpan,
    },
    /// Variable definition syntax.
    Variable {
        /// Variable name.
        name: String,
        /// Assignment operator.
        operator: String,
        /// Unexpanded value.
        raw_value: String,
        /// Whether exported.
        exported: bool,
        /// Whether overridden.
        overridden: bool,
        /// Whether a define block.
        define_block: bool,
        /// Conditional ancestry.
        conditions: Vec<ConditionObservation>,
        /// Complete definition span.
        span: SourceSpan,
    },
    /// Include directive syntax.
    Include {
        /// Unexpanded path expression.
        raw_path: String,
        /// Whether missing files are allowed.
        optional: bool,
        /// Conditional ancestry.
        conditions: Vec<ConditionObservation>,
        /// Complete directive span.
        span: SourceSpan,
    },
    /// Parser diagnostic.
    Diagnostic {
        /// Human-readable message.
        message: String,
        /// Optional upstream code.
        code: Option<String>,
        /// Positioned or derived span.
        span: SourceSpan,
    },
}

/// Parser adapter invariant failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParserPortError {
    /// The upstream tree did not render back to the original source.
    #[error("upstream concrete syntax tree did not round-trip")]
    RoundTripMismatch,
    /// An upstream range violated the source contract.
    #[error(transparent)]
    InvalidLocation(#[from] LocationError),
    /// A required syntax accessor was absent.
    #[error("required {field} accessor was absent")]
    MissingField {
        /// Stable semantic field name.
        field: &'static str,
    },
}

/// Parses source without exposing upstream parser types.
pub trait MakefileParser {
    /// Collect ordered GNU Make syntax observations.
    ///
    /// # Errors
    ///
    /// Returns [`ParserPortError`] when an adapter invariant is violated.
    fn parse(&self, source: &str) -> Result<ParserOutcome, ParserPortError>;
}
