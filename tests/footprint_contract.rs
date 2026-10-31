//! Contract for the development build footprint (Concordat rule
//! `rust-dev-build-footprint`): the default development and test profiles keep
//! no incremental state and no debug information, and Cargo coordinates
//! metadata output through its native key rather than a raw rustc flag.
//!
//! The checks ask Cargo itself for the effective values, so a profile override
//! in any configuration file Cargo merges (this repository's, an ancestor
//! directory's or the user's) is seen as well as the manifest's own table.
//! Nothing here compiles: `--unit-graph` and `config get` resolve and print.

use std::process::Command;

/// Profile overrides that the environment can carry; removed so the
/// repository's own settings are what is measured.
const PROFILE_VARIABLES: &[&str] = &[
    "CARGO_INCREMENTAL",
    "CARGO_PROFILE_DEV_DEBUG",
    "CARGO_PROFILE_DEV_INCREMENTAL",
    "CARGO_PROFILE_TEST_DEBUG",
    "CARGO_PROFILE_TEST_INCREMENTAL",
];

/// Runs the Cargo that built this test in the crate directory and returns its
/// standard output.
fn cargo(args: &[&str]) -> Result<String, String> {
    let mut command = Command::new(env!("CARGO"));
    command.args(args).current_dir(env!("CARGO_MANIFEST_DIR"));
    for name in PROFILE_VARIABLES {
        command.env_remove(name);
    }
    let output = command
        .output()
        .map_err(|error| format!("running cargo {args:?}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

/// Returns an error carrying the message unless the condition holds.
fn ensure(holds: bool, message: impl FnOnce() -> String) -> Result<(), String> {
    if holds { Ok(()) } else { Err(message()) }
}

/// Returns the profile objects of a `--unit-graph` document: the text after
/// each `"profile":{`, up to the first closing brace.
fn profiles(graph: &str) -> Vec<&str> {
    graph
        .split("\"profile\":{")
        .skip(1)
        .filter_map(|rest| rest.split_once('}').map(|(profile, _)| profile))
        .collect()
}

/// Checks that every unit of the graph disables debug information and
/// incremental state.
fn check_footprint_profiles(graph: &str, what: &str) -> Result<(), String> {
    let found = profiles(graph);
    ensure(!found.is_empty(), || {
        format!("{what}: the unit graph names no profile")
    })?;
    for profile in found {
        ensure(profile.contains("\"debuginfo\":0,"), || {
            format!("{what}: a unit keeps debug information: {profile}")
        })?;
        ensure(profile.contains("\"incremental\":false,"), || {
            format!("{what}: a unit keeps incremental state: {profile}")
        })?;
    }
    Ok(())
}

#[test]
fn the_effective_development_profile_disables_incremental_state_and_debug_information()
-> Result<(), String> {
    let graph = cargo(&["build", "--unit-graph", "-Zunstable-options", "--offline"])?;
    check_footprint_profiles(&graph, "cargo build")
}

#[test]
fn the_effective_test_profile_inherits_the_footprint_settings() -> Result<(), String> {
    let graph = cargo(&[
        "test",
        "--no-run",
        "--unit-graph",
        "-Zunstable-options",
        "--offline",
    ])?;
    check_footprint_profiles(&graph, "cargo test")
}

#[test]
fn cargo_reads_the_native_key_for_metadata_embedding() -> Result<(), String> {
    let config = cargo(&[
        "config",
        "get",
        "--format",
        "json",
        "-Zunstable-options",
        "unstable.embed-metadata",
    ])?;
    ensure(
        config.trim() == r#"{"unstable":{"embed-metadata":false}}"#,
        || {
            format!(
                "[unstable] must set embed-metadata = false on a nightly from 2026-07-31; Cargo \
                 reads {config}"
            )
        },
    )
}

#[test]
fn no_raw_rustc_flag_stands_in_for_the_native_key() {
    let config = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/.cargo/config.toml"));
    assert!(
        !config
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .any(|line| line.contains("no-embed-metadata") || line.contains("-Zembed-metadata")),
        "metadata embedding is set through Cargo, not a raw rustc flag"
    );
}
