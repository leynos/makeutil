//! JSON Schema and snapshot tests for the stable report contract.

use makeutil::{adapters::MakefileLosslessParser, parse_source};
use rstest::rstest;

fn schema() -> Result<serde_json::Value, serde_json::Error> {
    serde_json::from_str(include_str!("../schemas/makeutil.parse.v1.schema.json"))
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
fn all_fact_variants_have_stable_json() -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_source(
        include_bytes!("fixtures/makefiles/all-facts.mk"),
        "Makefile",
        &MakefileLosslessParser,
    )?;
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
