//! Black-box process contract tests for the public executable.

mod common;

use std::io::Write as _;

use assert_cmd::Command;
use camino::Utf8Path;
use common::MockSourceReader;
use makeutil::adapters::{
    cli::{ProcessCapabilities, run_from, run_from_with_reader},
    source::MAX_SOURCE_BYTES,
};
use rstest::{fixture, rstest};

#[fixture]
fn makeutil_command() -> Command {
    let binary = assert_cmd::cargo::cargo_bin!("makeutil");
    Command::new(binary)
}

#[rstest]
fn complete_path_emits_one_json_document(mut makeutil_command: Command) {
    let output = makeutil_command
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
fn recovered_path_exits_one_with_json(mut makeutil_command: Command) {
    let output = makeutil_command
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
#[case(&["parse", "-"][..], "--stdin-filename")]
#[case(&["parse"][..], "Usage:")]
#[case(
    &["parse", "--stdin-filename", "Makefile", "ordinary.mk"][..],
    "only valid when PATH is -"
)]
fn invalid_invocation_exits_two(
    mut makeutil_command: Command,
    #[case] arguments: &[&str],
    #[case] expected_detail: &str,
) {
    let output = makeutil_command
        .args(arguments)
        .output()
        .expect("binary should run");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.starts_with(b"makeutil: cli:"));
    assert!(String::from_utf8_lossy(&output.stderr).contains(expected_detail));
}

#[rstest]
fn help_uses_clap_display_stream(mut makeutil_command: Command) {
    let output = makeutil_command
        .arg("--help")
        .output()
        .expect("binary should run");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_err());
}

#[rstest]
fn version_uses_clap_display_stream(mut makeutil_command: Command) {
    let output = makeutil_command
        .arg("--version")
        .output()
        .expect("binary should run");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8_lossy(&output.stdout).contains(env!("CARGO_PKG_VERSION")));
    assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_err());
}

#[rstest]
fn hostile_source_is_inert(mut makeutil_command: Command) {
    let temporary = tempfile::tempdir().expect("temporary directory should exist");
    let shell_sentinel = temporary.path().join("shell-sentinel");
    let file_sentinel = temporary.path().join("file-sentinel");
    let assignment_sentinel = temporary.path().join("assignment-sentinel");
    let recipe_sentinel = temporary.path().join("recipe-sentinel");
    let source = format!(
        concat!(
            "SHELL := $(shell touch {})\n",
            "FILE := $(file >{},created)\n",
            "ASSIGNMENT != touch {}\n",
            "all:\n\ttouch {}\n",
        ),
        shell_sentinel.display(),
        file_sentinel.display(),
        assignment_sentinel.display(),
        recipe_sentinel.display(),
    );
    let output = makeutil_command
        .args(["parse", "--stdin-filename", "Makefile", "-"])
        .write_stdin(source)
        .output()
        .expect("binary should run");
    assert_eq!(output.status.code(), Some(0));
    for sentinel in [
        shell_sentinel,
        file_sentinel,
        assignment_sentinel,
        recipe_sentinel,
    ] {
        assert!(
            !sentinel.exists(),
            "{} should not exist",
            sentinel.display()
        );
    }
}

#[rstest]
fn include_paths_are_not_opened_and_caller_path_spelling_is_preserved() {
    let caller_path = "./fixtures/../caller.mk";
    let mut source_reader = MockSourceReader::new();
    source_reader
        .expect_open()
        .withf(move |path| path == Utf8Path::new(caller_path))
        .times(1)
        .returning(|_| {
            Ok(Box::new(std::io::Cursor::new(
                b"include absent.mk\ninclude $(CONFIG_DIR)/dynamic.mk\nall:\n".as_slice(),
            )))
        });
    let mut stdin = std::io::empty();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let capabilities =
        ProcessCapabilities::new(&mut stdin, &mut stdout, &mut stderr, &source_reader);

    let outcome = run_from_with_reader(["makeutil", "parse", caller_path], capabilities);

    assert_eq!(outcome.exit_code, 0);
    assert!(stderr.is_empty());
    let document: serde_json::Value =
        serde_json::from_slice(&stdout).expect("stdout should be JSON");
    assert_eq!(
        document
            .pointer("/source/path")
            .and_then(serde_json::Value::as_str),
        Some(caller_path)
    );
    assert_eq!(
        document
            .get("includes")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[rstest]
fn environment_and_config_cannot_supply_parse_arguments(mut makeutil_command: Command) {
    let temporary = tempfile::tempdir().expect("temporary directory should exist");
    let mut config = tempfile::NamedTempFile::new_in(temporary.path())
        .expect("configuration fixture should be created");
    config
        .write_all(b"path = \"from-config.mk\"\nstdin_filename = \"config-logical.mk\"\n")
        .expect("configuration fixture should be written");

    let output = makeutil_command
        .current_dir(temporary.path())
        .env("MAKEUTIL_PARSE_PATH", "from-environment.mk")
        .env("MAKEUTIL_PARSE_STDIN_FILENAME", "environment-logical.mk")
        .env("MAKEUTIL_PARSE_CONFIG_PATH", config.path())
        .arg("parse")
        .output()
        .expect("binary should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("<PATH>"));
}

#[rstest]
fn control_characters_cannot_inject_stderr_lines() {
    let logical_path = "logical.mk\nforged\t\u{1b}\u{85}";
    let mut stdin = std::io::repeat(b'x');
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let outcome = run_from(
        ["makeutil", "parse", "--stdin-filename", logical_path, "-"],
        &mut stdin,
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(outcome.exit_code, 2);
    assert!(stdout.is_empty());
    assert_eq!(
        String::from_utf8(stderr).expect("stderr should be UTF-8"),
        format!(
            "makeutil: source-too-large: source logical.mk\\nforged\\t\\u{{1b}}\\u{{85}} exceeds \
             the {MAX_SOURCE_BYTES}-byte limit\n"
        )
    );
}

#[rstest]
fn invalid_utf8_is_a_fatal_source_error(mut makeutil_command: Command) {
    let mut source = tempfile::NamedTempFile::new().expect("temporary file should exist");
    source
        .write_all(&[0xff])
        .expect("fixture bytes should be written");
    let path = source
        .path()
        .to_str()
        .expect("temporary path should be UTF-8");
    let output = makeutil_command
        .args(["parse", path])
        .output()
        .expect("binary should run");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("makeutil: source-utf8:"));
}
