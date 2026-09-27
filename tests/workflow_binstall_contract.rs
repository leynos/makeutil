//! Contract test that CI's tool installs through cargo-binstall fail closed.
//!
//! Unauthenticated, binstall's lookup of a tool's GitHub release can be
//! refused with HTTP 403. With its compile strategy enabled it then falls
//! back to building the tool from source, which for nextest fails outright
//! (nextest refuses an unlocked build) and for any tool costs minutes. Each
//! install step in `ci.yml` therefore passes the workflow token and disables
//! the compile strategy, so a missing prebuilt is a loud failure rather than a
//! silent rebuild. These tests hold both halves on each step.

use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;

/// The exact environment line that authenticates binstall's GitHub lookups.
const TOKEN_LINE: &str = "GITHUB_TOKEN: ${{ github.token }}";

/// Reads `.github/workflows/ci.yml` under the crate directory.
fn ci_workflow() -> std::io::Result<String> {
    Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())?
        .read_to_string(".github/workflows/ci.yml")
}

/// Returns the trimmed lines of the step named `name`: from its `- name:`
/// line up to the next list item at the same indentation.
fn step_lines<'a>(workflow: &'a str, name: &str) -> Vec<&'a str> {
    let heading = format!("- name: {name}");
    let mut lines = workflow.lines();
    let Some(start) = lines.find(|line| line.trim() == heading) else {
        return Vec::new();
    };
    let indent = start.len() - start.trim_start().len();
    let mut found = vec![start.trim()];
    found.extend(
        lines
            .take_while(|line| {
                let is_next_item = line.trim_start().starts_with("- ")
                    && line.len() - line.trim_start().len() == indent;
                !is_next_item
            })
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#')),
    );
    found
}

/// Each tool install passes the workflow token and runs exactly the
/// fail-closed command, with the compile strategy disabled.
#[rstest]
#[case::test_runner("Install test runner", "cargo-nextest")]
#[case::audit("Install cargo-audit", "cargo-audit")]
fn binstall_steps_are_authenticated_and_fail_closed(
    #[case] step: &str,
    #[case] tool: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let workflow = ci_workflow()?;
    let lines = step_lines(&workflow, step);
    let command = format!("run: cargo binstall --no-confirm --disable-strategies compile {tool}");

    if lines.is_empty() {
        return Err(format!("ci.yml must have a step named {step:?}").into());
    }
    if !lines.contains(&TOKEN_LINE) {
        return Err(format!(
            "{step:?} must set {TOKEN_LINE:?} so binstall's lookups are authenticated: {lines:?}"
        )
        .into());
    }
    if !lines.contains(&command.as_str()) {
        return Err(format!(
            "{step:?} must run {command:?}, so a missing prebuilt fails the step: {lines:?}"
        )
        .into());
    }
    Ok(())
}
