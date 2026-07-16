//! Behavioural acceptance tests for the parse command.

mod common;

use std::io::Cursor;

use camino::Utf8Path;
use common::MockSourceReader;
use makeutil::adapters::cli::{ProcessCapabilities, run_from_with_reader};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct World {
    arguments: Vec<String>,
    stdin: Vec<u8>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: Option<u8>,
}

#[fixture]
fn world() -> World {
    World {
        arguments: Vec::new(),
        stdin: Vec::new(),
        stdout: Vec::new(),
        stderr: Vec::new(),
        exit_code: None,
    }
}

#[given("a complete GNU Makefile fixture")]
fn complete_fixture(world: &mut World) {
    world.arguments = vec![
        "makeutil".to_owned(),
        "parse".to_owned(),
        "tests/fixtures/makefiles/all-facts.mk".to_owned(),
    ];
}

#[given("complete GNU Makefile source on standard input")]
fn complete_stdin(world: &mut World) { world.stdin = b"all:\n\techo ok\n".to_vec(); }

#[given("a path that does not exist")]
fn missing_path(world: &mut World) {
    world.arguments = vec![
        "makeutil".to_owned(),
        "parse".to_owned(),
        "tests/fixtures/makefiles/absent.mk".to_owned(),
    ];
}

#[given("an invalid parse invocation")]
fn invalid_invocation(world: &mut World) {
    world.arguments = vec!["makeutil".to_owned(), "parse".to_owned(), "-".to_owned()];
}

#[given("a help display request")]
fn help_request(world: &mut World) {
    world.arguments = vec!["makeutil".to_owned(), "--help".to_owned()];
}

#[given("a version display request")]
fn version_request(world: &mut World) {
    world.arguments = vec!["makeutil".to_owned(), "--version".to_owned()];
}

#[when("makeutil parses the fixture by path")]
#[when("makeutil attempts to parse the missing path")]
#[when("makeutil processes the invocation")]
fn run_path(world: &mut World) { run_world(world); }

#[when("makeutil parses dash with stdin filename Makefile")]
fn run_stdin(world: &mut World) {
    world.arguments = vec![
        "makeutil".to_owned(),
        "parse".to_owned(),
        "--stdin-filename".to_owned(),
        "Makefile".to_owned(),
        "-".to_owned(),
    ];
    run_world(world);
}

#[then("stdout contains one schema version 1 JSON document")]
fn schema_version(world: &World) -> googletest::Result<()> {
    let document: serde_json::Value = serde_json::from_slice(&world.stdout)?;
    assert_eq!(
        document
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    Ok(())
}

#[then("the report source path is Makefile")]
fn stdin_path(world: &World) -> googletest::Result<()> {
    let document: serde_json::Value = serde_json::from_slice(&world.stdout)?;
    assert_eq!(
        document
            .pointer("/source/path")
            .and_then(serde_json::Value::as_str),
        Some("Makefile")
    );
    Ok(())
}

#[then("the process exits with code {expected:u8}")]
fn exit_code(world: &World, expected: u8) {
    assert_eq!(world.exit_code, Some(expected));
}

#[then("stderr is empty")]
fn empty_stderr(world: &World) {
    assert!(world.stderr.is_empty());
}

#[then("stdout is empty")]
fn empty_stdout(world: &World) {
    assert!(world.stdout.is_empty());
}

#[then("stderr reports the source-open operation")]
fn source_open(world: &World) {
    assert!(String::from_utf8_lossy(&world.stderr).contains("makeutil: source-open:"));
}

#[then("stderr reports the cli operation")]
fn cli_error(world: &World) {
    assert!(String::from_utf8_lossy(&world.stderr).contains("makeutil: cli:"));
}

#[then("stdout contains command help")]
fn command_help(world: &World) {
    assert!(String::from_utf8_lossy(&world.stdout).contains("Usage:"));
}

#[then("stdout contains the makeutil version")]
fn command_version(world: &World) {
    assert!(String::from_utf8_lossy(&world.stdout).contains(env!("CARGO_PKG_VERSION")));
}

fn run_world(world: &mut World) {
    let mut source_reader = MockSourceReader::new();
    source_reader.expect_open().returning(|path| {
        if path == Utf8Path::new("tests/fixtures/makefiles/all-facts.mk") {
            return Ok(Box::new(Cursor::new(include_bytes!(
                "fixtures/makefiles/all-facts.mk"
            ))));
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "fixture is absent",
        ))
    });
    let mut stdin = world.stdin.as_slice();
    let capabilities = ProcessCapabilities::new(
        &mut stdin,
        &mut world.stdout,
        &mut world.stderr,
        &source_reader,
    );
    let outcome = run_from_with_reader(world.arguments.clone(), capabilities);
    world.exit_code = Some(outcome.exit_code);
}

#[scenario(
    path = "tests/features/parse.feature",
    name = "Parse a complete Makefile by path"
)]
fn parse_path(_world: World) {}

#[scenario(
    path = "tests/features/parse.feature",
    name = "Parse complete source from standard input"
)]
fn parse_stdin(_world: World) {}

#[scenario(
    path = "tests/features/parse.feature",
    name = "Reject a missing input path"
)]
fn reject_missing(_world: World) {}

#[scenario(
    path = "tests/features/parse.feature",
    name = "Reject an invalid invocation"
)]
fn reject_invalid_invocation(_world: World) {}

#[scenario(path = "tests/features/parse.feature", name = "Display help")]
fn display_help(_world: World) {}

#[scenario(path = "tests/features/parse.feature", name = "Display version")]
fn display_version(_world: World) {}
