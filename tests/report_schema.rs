//! JSON Schema and snapshot tests for the stable report contract.

use makeutil::{
    ParseApplicationError,
    adapters::MakefileLosslessParser,
    domain::ParseReport,
    parse_source,
};
use pretty_assertions::assert_eq;
use rstest::{fixture, rstest};

#[derive(Debug, serde::Deserialize)]
struct ConsumerReport {
    schema_version: u64,
    parse: ConsumerParse,
    rules: Vec<ConsumerRule>,
}

#[derive(Debug, serde::Deserialize)]
struct ConsumerParse {
    status: String,
}

#[derive(Debug, serde::Deserialize)]
struct ConsumerRule {
    targets: Vec<String>,
}

fn schema() -> Result<serde_json::Value, serde_json::Error> {
    serde_json::from_str(include_str!("../schemas/makeutil.parse.v1.schema.json"))
}

#[fixture]
fn all_facts_report() -> Result<ParseReport, ParseApplicationError> {
    parse_source(
        include_bytes!("fixtures/makefiles/all-facts.mk"),
        "Makefile",
        &MakefileLosslessParser,
    )
}

#[rstest]
#[case(include_bytes!("fixtures/makefiles/all-facts.mk"), "complete.mk")]
#[case(include_bytes!("fixtures/makefiles/recovered.mk"), "recovered.mk")]
fn reports_validate_against_schema(
    #[case] source: &[u8],
    #[case] path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(source, path, &MakefileLosslessParser)?;
    let document = serde_json::to_value(report)?;
    let validator = jsonschema::validator_for(&schema()?)?;
    if validator.is_valid(&document) {
        Ok(())
    } else {
        Err("report did not validate against schema".into())
    }
}

#[rstest]
fn malformed_near_miss_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let validator = jsonschema::validator_for(&schema()?)?;
    let malformed = serde_json::json!({"schema_version": 1, "unexpected": true});
    if validator.is_valid(&malformed) {
        Err("malformed near-miss unexpectedly validated".into())
    } else {
        Ok(())
    }
}

#[rstest]
#[case::complete_with_diagnostics(true)]
#[case::recovered_without_diagnostics(false)]
fn contradictory_parse_summaries_are_rejected(
    #[case] complete_with_diagnostics: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let complete = parse_source(
        include_bytes!("fixtures/makefiles/all-facts.mk"),
        "Makefile",
        &MakefileLosslessParser,
    )?;
    let recovered = parse_source(
        include_bytes!("fixtures/makefiles/recovered.mk"),
        "recovered.mk",
        &MakefileLosslessParser,
    )?;
    let recovered_diagnostics = serde_json::to_value(&recovered.parse.diagnostics)?;
    let mut document = serde_json::to_value(if complete_with_diagnostics {
        complete
    } else {
        recovered
    })?;
    let diagnostics = if complete_with_diagnostics {
        recovered_diagnostics
    } else {
        serde_json::json!([])
    };
    let diagnostics_slot = document
        .pointer_mut("/parse/diagnostics")
        .ok_or("serialized report should contain parse diagnostics")?;
    *diagnostics_slot = diagnostics;

    let validator = jsonschema::validator_for(&schema()?)?;
    if validator.is_valid(&document) {
        Err("contradictory parse summary unexpectedly validated".into())
    } else {
        Ok(())
    }
}

#[rstest]
fn independent_consumer_deserializes_schema_v1(
    all_facts_report: Result<ParseReport, ParseApplicationError>,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = all_facts_report?;
    let document = serde_json::to_vec(&report)?;
    let consumer: ConsumerReport = serde_json::from_slice(&document)?;

    assert_eq!(consumer.schema_version, 1);
    assert_eq!(consumer.parse.status, "complete");
    assert_eq!(
        consumer
            .rules
            .first()
            .and_then(|rule| rule.targets.first())
            .map(String::as_str),
        Some("check")
    );
    Ok(())
}

#[rstest]
fn all_fact_variants_have_stable_json(
    all_facts_report: Result<ParseReport, ParseApplicationError>,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = all_facts_report?;
    insta::assert_json_snapshot!(report);
    Ok(())
}

#[rstest]
fn recovered_output_has_stable_json() -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(
        include_bytes!("fixtures/makefiles/recovered.mk"),
        "recovered.mk",
        &MakefileLosslessParser,
    )?;
    insta::assert_json_snapshot!(report);
    Ok(())
}
