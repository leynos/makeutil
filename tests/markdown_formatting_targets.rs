//! Behavioural tests for the `fmt` and `check-fmt` targets of the Makefile.
//!
//! The wiring contracts in `workflow_suite_contract.rs` read the recipes as text. These tests run
//! the shipped recipes: each case builds a throwaway Git repository holding a copy of the real
//! Makefile and a tracked and an untracked Markdown file, puts recording stubs in place of
//! `cargo`, `mdtablefix` and `markdownlint-cli2` through the Makefile's own variables, and runs
//! `make fmt` or `make check-fmt`. The log of stub calls shows the arguments each tool received,
//! their order, and whether a tool's failing exit status reaches Make.

use std::{
    io,
    path::Path,
    process::{Command, Output},
};

use camino::Utf8Path;
use cap_std::{
    ambient_authority,
    fs::{Permissions, PermissionsExt},
    fs_utf8::Dir,
};
use rstest::rstest;
use tempfile::TempDir;

/// The five rewrite flags and two selection flags every invocation carries.
const SHARED: &str = "--git --include-untracked --wrap --renumber --breaks --ellipsis --fences";

/// A throwaway repository with recording stubs on its `bin` path.
struct Scratch {
    repository: TempDir,
    dir: Dir,
}

impl Scratch {
    /// Builds the repository: the real Makefile, one tracked and one untracked Markdown file, and
    /// a stub for each tool that exits with the status `failing` names (`None` for success).
    fn new(failing: Option<&str>) -> io::Result<Self> {
        let repository = TempDir::new()?;
        let root = Utf8Path::from_path(repository.path())
            .ok_or_else(|| io::Error::other("the temporary directory is not UTF-8"))?;
        let source = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())?;
        let dir = Dir::open_ambient_dir(root, ambient_authority())?;
        dir.write("Makefile", source.read("Makefile")?)?;
        dir.write("tracked.md", "# Tracked\n")?;
        dir.create_dir("bin")?;
        for tool in ["cargo", "mdtablefix", "markdownlint-cli2"] {
            let code = i32::from(failing == Some(tool));
            let script =
                format!("#!/bin/sh\nprintf '%s %s\\n' '{tool}' \"$*\" >> \"$LOG\"\nexit {code}\n");
            let path = format!("bin/{tool}");
            dir.write(&path, script)?;
            dir.set_permissions(&path, Permissions::from_mode(0o755))?;
        }
        for args in [&["init", "--quiet"][..], &["add", "tracked.md"][..]] {
            Self::git(repository.path(), args)?;
        }
        dir.write("untracked.md", "# Untracked\n")?;
        Ok(Self { repository, dir })
    }

    /// Runs Git in `dir` with the ambient Git environment cleared.
    fn git(dir: &Path, args: &[&str]) -> io::Result<()> {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .status()?;
        status
            .success()
            .then_some(())
            .ok_or_else(|| io::Error::other(format!("git {args:?} failed")))
    }

    /// Runs `make <target>` with the stubs substituted for the tools.
    fn make(&self, target: &str) -> io::Result<Output> {
        let bin = self.repository.path().join("bin");
        let stub = |name: &str| format!("{}/{name}", bin.display());
        Command::new("make")
            .arg("--no-print-directory")
            .arg(format!("CARGO={}", stub("cargo")))
            .arg(format!("MDTABLEFIX={}", stub("mdtablefix")))
            .arg(format!("MDLINT={}", stub("markdownlint-cli2")))
            .arg(target)
            .current_dir(self.repository.path())
            .env("LOG", self.repository.path().join("log"))
            .output()
    }

    /// Returns the stub calls the recipes made, in order. The Makefile asks `cargo` whether
    /// nextest is present while it is read; that probe is not a recipe call.
    fn calls(&self) -> Vec<String> {
        let text = self.dir.read_to_string("log").unwrap_or_default();
        text.lines()
            .filter(|line| !line.starts_with("cargo nextest"))
            .map(str::to_owned)
            .collect()
    }
}

#[test]
fn check_fmt_runs_the_rust_check_then_mdtablefix_in_check_mode() {
    let scratch = Scratch::new(None).expect("the scratch repository builds");

    let output = scratch.make("check-fmt").expect("make starts");

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        scratch.calls(),
        vec![
            "cargo fmt --all -- --check".to_owned(),
            format!("mdtablefix --check {SHARED}"),
        ]
    );
}

#[test]
fn fmt_rewrites_then_runs_the_linter_with_fix_last() {
    let scratch = Scratch::new(None).expect("the scratch repository builds");

    let output = scratch.make("fmt").expect("make starts");

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        scratch.calls(),
        vec![
            "cargo +nightly fmt --all".to_owned(),
            format!("mdtablefix --in-place {SHARED}"),
            "markdownlint-cli2 --fix **/*.md".to_owned(),
        ]
    );
}

#[rstest]
#[case::check_fmt_mdtablefix("check-fmt", "mdtablefix")]
#[case::check_fmt_cargo("check-fmt", "cargo")]
#[case::fmt_mdtablefix("fmt", "mdtablefix")]
#[case::fmt_linter("fmt", "markdownlint-cli2")]
#[case::fmt_cargo("fmt", "cargo")]
fn a_failing_tool_fails_the_target(#[case] target: &str, #[case] failing: &str) {
    let scratch = Scratch::new(Some(failing)).expect("the scratch repository builds");

    let output = scratch.make(target).expect("make starts");

    assert!(
        !output.status.success(),
        "a failing {failing} did not fail make {target}"
    );
}

#[test]
fn a_failing_mdtablefix_stops_fmt_before_the_linter_runs() {
    let scratch = Scratch::new(Some("mdtablefix")).expect("the scratch repository builds");

    scratch.make("fmt").expect("make starts");

    let calls = scratch.calls();
    assert!(
        !calls
            .iter()
            .any(|call| call.starts_with("markdownlint-cli2")),
        "the linter ran after a failed rewrite: {calls:?}"
    );
}
