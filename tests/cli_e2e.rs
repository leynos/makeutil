//! Black-box process contract tests for the public executable.

use std::io::Write as _;

use assert_cmd::Command;
use rstest::rstest;

#[rstest]
fn complete_path_emits_one_json_document() {
    let output = Command::cargo_bin("makeutil")
        .expect("binary should build")
        .args(["parse", "tests/fixtures/makefiles/all-facts.mk"])
        .output()
        .expect("binary should run");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        document
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[rstest]
fn recovered_path_exits_one_with_json() {
    let output = Command::cargo_bin("makeutil")
        .expect("binary should build")
        .args(["parse", "tests/fixtures/makefiles/recovered.mk"])
        .output()
        .expect("binary should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        document
            .pointer("/parse/status")
            .and_then(serde_json::Value::as_str),
        Some("recovered")
    );
}

#[rstest]
#[case(&["parse", "-"][..])]
#[case(&["parse"][..])]
fn invalid_invocation_exits_two(#[case] arguments: &[&str]) {
    let output = Command::cargo_bin("makeutil")
        .expect("binary should build")
        .args(arguments)
        .output()
        .expect("binary should run");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[rstest]
fn hostile_source_is_inert() {
    let temporary = tempfile::tempdir().expect("temporary directory should exist");
    let sentinel = temporary.path().join("sentinel");
    let source = format!(
        "X := $(shell touch {})\nall:\n\ttouch {}\n",
        sentinel.display(),
        sentinel.display()
    );
    let mut command = Command::cargo_bin("makeutil").expect("binary should build");
    let output = command
        .args(["parse", "--stdin-filename", "Makefile", "-"])
        .write_stdin(source)
        .output()
        .expect("binary should run");
    assert_eq!(output.status.code(), Some(0));
    assert!(!sentinel.exists());
}

#[rstest]
fn invalid_utf8_is_a_fatal_source_error() {
    let mut source = tempfile::NamedTempFile::new().expect("temporary file should exist");
    source
        .write_all(&[0xff])
        .expect("fixture bytes should be written");
    let path = source
        .path()
        .to_str()
        .expect("temporary path should be UTF-8");
    let output = Command::cargo_bin("makeutil")
        .expect("binary should build")
        .args(["parse", path])
        .output()
        .expect("binary should run");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("makeutil: source-utf8:"));
}
