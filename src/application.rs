//! Application policy for validating bytes and assembling a stable report.

use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::{
    domain::{
        AssignmentOperator,
        ConditionContext,
        IncludeFact,
        LocationError,
        LocationIndex,
        ParseDiagnostic,
        ParseReport,
        ParseStatus,
        ParseSummary,
        RecipeFact,
        RuleFact,
        SCHEMA_VERSION,
        SourceIdentity,
        ToolIdentity,
        VariableFact,
    },
    ports::{ConditionObservation, MakefileParser, ParserPortError, SyntaxObservation},
};

/// Failure before a report can be produced.
#[derive(Debug, Error)]
pub enum ParseApplicationError {
    /// Source bytes are not UTF-8.
    #[error("source is not valid UTF-8: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),
    /// Parser adapter invariant failed.
    #[error(transparent)]
    Parser(#[from] ParserPortError),
    /// A parser-owned span could not be mapped.
    #[error(transparent)]
    Location(#[from] LocationError),
}

/// Parse exact bytes under a caller-supplied logical source name.
///
/// # Errors
///
/// Returns [`ParseApplicationError`] for invalid UTF-8 or parser invariants.
///
/// # Examples
///
/// ```
/// use makeutil::{adapters::MakefileLosslessParser, parse_source};
///
/// let report = parse_source(b"all:\n\techo ok\n", "Makefile", &MakefileLosslessParser)?;
/// assert_eq!(
///     report
///         .rules
///         .first()
///         .and_then(|rule| rule.targets.first())
///         .map(String::as_str),
///     Some("all"),
/// );
/// # Ok::<(), makeutil::ParseApplicationError>(())
/// ```
pub fn parse_source(
    source: &[u8],
    logical_path: &str,
    parser: &impl MakefileParser,
) -> Result<ParseReport, ParseApplicationError> {
    let source_text = std::str::from_utf8(source)?;
    let outcome = parser.parse(source_text)?;
    let locations = LocationIndex::new(source_text);
    let mut assembly = ReportAssembly::default();
    for observation in outcome.observations {
        assembly.push(observation, &locations)?;
    }
    let status = assembly.status();
    Ok(ParseReport {
        schema_version: SCHEMA_VERSION,
        tool: ToolIdentity::default(),
        source: SourceIdentity {
            path: logical_path.to_owned(),
            sha256: data_encoding::HEXLOWER.encode(&Sha256::digest(source)),
            byte_length: source.len(),
        },
        parse: ParseSummary {
            status,
            diagnostics: assembly.diagnostics,
        },
        rules: assembly.rules,
        variables: assembly.variables,
        includes: assembly.includes,
    })
}

#[derive(Default)]
struct ReportAssembly {
    rules: Vec<RuleFact>,
    variables: Vec<VariableFact>,
    includes: Vec<IncludeFact>,
    diagnostics: Vec<ParseDiagnostic>,
    fact_ordinal: usize,
}

impl ReportAssembly {
    fn push(
        &mut self,
        observation: SyntaxObservation,
        locations: &LocationIndex<'_>,
    ) -> Result<(), LocationError> {
        match observation {
            SyntaxObservation::Rule {
                targets,
                prerequisites,
                double_colon,
                conditions,
                recipes,
                span,
            } => self.push_rule(
                RuleParts {
                    targets,
                    prerequisites,
                    double_colon,
                    conditions,
                    recipes,
                    span,
                },
                locations,
            ),
            SyntaxObservation::Variable {
                name,
                operator,
                raw_value,
                exported,
                overridden,
                define_block,
                conditions,
                span,
            } => self.push_variable(
                VariableParts {
                    name,
                    operator,
                    raw_value,
                    exported,
                    overridden,
                    define_block,
                    conditions,
                    span,
                },
                locations,
            ),
            SyntaxObservation::Include {
                raw_path,
                optional,
                conditions,
                span,
            } => self.push_include(
                IncludeParts {
                    raw_path,
                    optional,
                    conditions,
                    span,
                },
                locations,
            ),
            SyntaxObservation::Diagnostic {
                message,
                code,
                span,
            } => {
                self.diagnostics.push(ParseDiagnostic {
                    message,
                    code,
                    location: locations.locate(span)?,
                });
                Ok(())
            }
        }
    }

    fn push_rule(
        &mut self,
        parts: RuleParts,
        locations: &LocationIndex<'_>,
    ) -> Result<(), LocationError> {
        let located_recipes = parts
            .recipes
            .into_iter()
            .enumerate()
            .map(|(ordinal, recipe)| {
                Ok(RecipeFact {
                    ordinal,
                    text: recipe.text,
                    silent: recipe.silent,
                    ignore_errors: recipe.ignore_errors,
                    always_execute: recipe.always_execute,
                    location: locations.locate(recipe.span)?,
                })
            })
            .collect::<Result<Vec<_>, LocationError>>()?;
        self.rules.push(RuleFact {
            ordinal: self.fact_ordinal,
            targets: parts.targets,
            prerequisites: parts.prerequisites,
            double_colon: parts.double_colon,
            conditions: locate_conditions(parts.conditions, locations)?,
            recipes: located_recipes,
            location: locations.locate(parts.span)?,
        });
        self.fact_ordinal += 1;
        Ok(())
    }

    fn push_variable(
        &mut self,
        parts: VariableParts,
        locations: &LocationIndex<'_>,
    ) -> Result<(), LocationError> {
        self.variables.push(VariableFact {
            ordinal: self.fact_ordinal,
            name: parts.name,
            operator: parts.operator,
            raw_value: parts.raw_value,
            exported: parts.exported,
            overridden: parts.overridden,
            define_block: parts.define_block,
            conditions: locate_conditions(parts.conditions, locations)?,
            location: locations.locate(parts.span)?,
        });
        self.fact_ordinal += 1;
        Ok(())
    }

    fn push_include(
        &mut self,
        parts: IncludeParts,
        locations: &LocationIndex<'_>,
    ) -> Result<(), LocationError> {
        self.includes.push(IncludeFact {
            ordinal: self.fact_ordinal,
            dynamic: parts.raw_path.contains('$'),
            raw_path: parts.raw_path,
            optional: parts.optional,
            conditions: locate_conditions(parts.conditions, locations)?,
            location: locations.locate(parts.span)?,
        });
        self.fact_ordinal += 1;
        Ok(())
    }

    const fn status(&self) -> ParseStatus {
        if self.diagnostics.is_empty() {
            ParseStatus::Complete
        } else {
            ParseStatus::Recovered
        }
    }
}

struct RuleParts {
    targets: Vec<String>,
    prerequisites: Vec<String>,
    double_colon: bool,
    conditions: Vec<ConditionObservation>,
    recipes: Vec<crate::ports::RecipeObservation>,
    span: crate::domain::SourceSpan,
}

struct VariableParts {
    name: String,
    operator: AssignmentOperator,
    raw_value: String,
    exported: bool,
    overridden: bool,
    define_block: bool,
    conditions: Vec<ConditionObservation>,
    span: crate::domain::SourceSpan,
}

struct IncludeParts {
    raw_path: String,
    optional: bool,
    conditions: Vec<ConditionObservation>,
    span: crate::domain::SourceSpan,
}

fn locate_conditions(
    conditions: Vec<ConditionObservation>,
    locations: &LocationIndex<'_>,
) -> Result<Vec<ConditionContext>, LocationError> {
    conditions
        .into_iter()
        .map(|condition| {
            Ok(ConditionContext {
                kind: condition.kind,
                expression: condition.expression,
                branch: condition.branch,
                location: locations.locate(condition.span)?,
            })
        })
        .collect()
}
