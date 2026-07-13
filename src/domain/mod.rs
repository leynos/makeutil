//! Stable report types owned by `makeutil` rather than its parser dependency.

mod location;

pub use location::{LocationError, LocationIndex, SourceLocation, SourceSpan};
use serde::Serialize;

/// Version of the JSON integration contract.
pub const SCHEMA_VERSION: u8 = 1;

/// Identity of the tool and parser used to produce a report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolIdentity {
    /// Executable name.
    pub name: &'static str,
    /// Executable package version.
    pub version: &'static str,
    /// Parser crate name.
    pub parser: &'static str,
    /// Exactly pinned parser crate version.
    pub parser_version: &'static str,
}

impl Default for ToolIdentity {
    fn default() -> Self {
        Self {
            name: "makeutil",
            version: env!("CARGO_PKG_VERSION"),
            parser: "makefile-lossless",
            parser_version: "0.3.40",
        }
    }
}

/// Exact identity of the parsed source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceIdentity {
    /// Caller-supplied logical source path.
    pub path: String,
    /// Lower-case hexadecimal SHA-256 digest of the exact bytes.
    pub sha256: String,
    /// Exact source length in bytes.
    pub byte_length: usize,
}

/// Whether parsing was complete or recovered through diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ParseStatus {
    /// No parser diagnostics were emitted.
    Complete,
    /// A partial syntax tree was recovered with diagnostics.
    Recovered,
}

/// One parser diagnostic attached to source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParseDiagnostic {
    /// Human-readable upstream message.
    pub message: String,
    /// Optional upstream diagnostic code.
    pub code: Option<String>,
    /// Source range associated with the problem.
    pub location: SourceLocation,
}

/// Parse classification and ordered diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParseSummary {
    /// Complete or recovered classification.
    pub status: ParseStatus,
    /// Diagnostics in upstream order.
    pub diagnostics: Vec<ParseDiagnostic>,
}

/// Conditional branch ancestry for a fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConditionContext {
    /// GNU Make conditional keyword.
    pub kind: String,
    /// Unexpanded condition expression.
    pub expression: String,
    /// Branch containing the fact.
    pub branch: ConditionBranch,
    /// Range of the opening or else directive.
    pub location: SourceLocation,
}

/// Branch of a conditional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConditionBranch {
    /// The opening conditional arm.
    If,
    /// The else arm.
    Else,
}

/// One source-faithful recipe line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecipeFact {
    /// Zero-based position within its rule.
    pub ordinal: usize,
    /// Recipe content without the leading tab or line ending.
    pub text: String,
    /// Whether the recipe has the `@` modifier.
    pub silent: bool,
    /// Whether the recipe has the `-` modifier.
    pub ignore_errors: bool,
    /// Whether the recipe has the `+` modifier.
    pub always_execute: bool,
    /// Complete physical recipe range.
    pub location: SourceLocation,
}

/// One explicit rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleFact {
    /// Global source-order position among facts.
    pub ordinal: usize,
    /// Unexpanded targets.
    pub targets: Vec<String>,
    /// Unexpanded prerequisites.
    pub prerequisites: Vec<String>,
    /// Whether the rule uses `::`.
    pub double_colon: bool,
    /// Outer-to-inner conditional ancestry.
    pub conditions: Vec<ConditionContext>,
    /// Recipes in source order.
    pub recipes: Vec<RecipeFact>,
    /// Complete rule range.
    pub location: SourceLocation,
}

/// One variable definition or define block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariableFact {
    /// Global source-order position among facts.
    pub ordinal: usize,
    /// Variable name.
    pub name: String,
    /// Source assignment operator.
    pub operator: String,
    /// Unexpanded source value.
    pub raw_value: String,
    /// Whether the `export` modifier is present.
    pub exported: bool,
    /// Whether the `override` modifier is present.
    pub overridden: bool,
    /// Whether this is a `define` block.
    pub define_block: bool,
    /// Outer-to-inner conditional ancestry.
    pub conditions: Vec<ConditionContext>,
    /// Complete definition range.
    pub location: SourceLocation,
}

/// One include directive, never followed by the parser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IncludeFact {
    /// Global source-order position among facts.
    pub ordinal: usize,
    /// Unexpanded include expression.
    pub raw_path: String,
    /// Whether a missing include is allowed.
    pub optional: bool,
    /// Whether the expression contains a Make expansion marker.
    pub dynamic: bool,
    /// Outer-to-inner conditional ancestry.
    pub conditions: Vec<ConditionContext>,
    /// Complete directive range.
    pub location: SourceLocation,
}

/// Versioned JSON document emitted by `makeutil parse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParseReport {
    /// Stable schema version, currently 1.
    pub schema_version: u8,
    /// Tool and parser identity.
    pub tool: ToolIdentity,
    /// Exact input identity.
    pub source: SourceIdentity,
    /// Parse status and diagnostics.
    pub parse: ParseSummary,
    /// Explicit rules in source order.
    pub rules: Vec<RuleFact>,
    /// Variable definitions in source order.
    pub variables: Vec<VariableFact>,
    /// Include directives in source order.
    pub includes: Vec<IncludeFact>,
}
