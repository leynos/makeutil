//! Behavioural tests for the `provenance` target of the Makefile.
//!
//! The target runs `git grep` over tracked sources, so each case builds a
//! throwaway Git repository holding a copy of the real Makefile and one
//! tracked file, then runs `make provenance` in it. That exercises the shipped
//! recipe, including its PCRE lookahead for GitHub Actions coordinates, rather
//! than a reimplementation of it.
//!
//! The owner name is assembled at run time. Writing it inline would put a
//! forbidden reference in this very file, which the target would reject.

use std::{
    io,
    path::Path,
    process::{Command, Output},
};

use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::prelude::*;
use rstest::rstest;
use tempfile::TempDir;

/// The organisation whose references the target polices.
const OWNER: &str = "leynos";

/// A repository name, shaped like the ones the coordinate pattern accepts.
const NAME: &str = "[a-z][a-z0-9_.-]{0,11}";

/// Run a command in `dir` with the ambient Git environment cleared.
///
/// `GIT_DIR` and its relatives would otherwise point Git at the checkout
/// running the tests rather than at the throwaway repository.
fn run_in(dir: &Path, program: &str, args: &[&str]) -> io::Result<Output> {
    Command::new(program)
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
}

/// Report whether `make provenance` accepts a repository whose only tracked
/// file is `file_name` with the given `content`.
fn provenance_accepts(file_name: &str, content: &str) -> io::Result<bool> {
    let repository = TempDir::new()?;
    let root = repository.path();
    let utf8_root = Utf8Path::from_path(root)
        .ok_or_else(|| io::Error::other("the temporary directory is not UTF-8"))?;
    let source = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())?;
    let target = Dir::open_ambient_dir(utf8_root, ambient_authority())?;
    target.write("Makefile", source.read("Makefile")?)?;
    target.write(file_name, content)?;
    for args in [&["init", "--quiet"][..], &["add", "--all"][..]] {
        if !run_in(root, "git", args)?.status.success() {
            return Err(io::Error::other(format!("git {args:?} failed")));
        }
    }
    Ok(
        run_in(root, "make", &["--no-print-directory", "provenance"])?
            .status
            .success(),
    )
}

/// What the target is expected to do with a repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    Accepted,
    Rejected,
}

/// Result type of a case, so a mismatch reads as a test failure with context.
type Outcome = Result<(), Box<dyn std::error::Error>>;

/// Fail unless the target gives `expected` for `content` in `file_name`.
fn expect(file_name: &str, content: &str, expected: Verdict) -> Outcome {
    let actual = if provenance_accepts(file_name, content)? {
        Verdict::Accepted
    } else {
        Verdict::Rejected
    };
    if actual == expected {
        return Ok(());
    }
    Err(format!("{file_name} with {content:?}: expected {expected:?}, got {actual:?}").into())
}

/// A valid action coordinate for `repository` and `action`.
fn coordinate(repository: &str, action: &str) -> String {
    format!("{OWNER}/{repository}/.github/actions/{action}@0123abc")
}

#[rstest]
#[case::coordinate(format!("uses: {}\n", coordinate("shared-actions", "generate-coverage")))]
#[case::coordinate_in_prose(format!("Pin {} here.\n", coordinate("some.repo_1", "x-y")))]
#[case::no_reference("Nothing to see.\n".to_owned())]
fn accepts_a_valid_action_coordinate(#[case] content: String) -> Outcome {
    expect("notes.md", &content, Verdict::Accepted)
}

#[rstest]
#[case::bare_repository(format!("See {OWNER}/shared-actions for details.\n"))]
#[case::issue_reference(format!("Fixes {OWNER}/shared-actions#12.\n"))]
#[case::url(format!("https://github.com/{OWNER}/makeutil\n"))]
#[case::workflow_path(format!("{OWNER}/shared-actions/.github/workflows/ci.yml@v1\n"))]
#[case::coordinate_without_ref(format!("{OWNER}/shared-actions/.github/actions/x\n"))]
#[case::wrong_directory(format!("{OWNER}/shared-actions/actions/x@v1\n"))]
#[case::coordinate_then_bare(format!(
    "{} and {OWNER}/other\n",
    coordinate("shared-actions", "x")
))]
#[case::bare_then_coordinate(format!(
    "{OWNER}/other and {}\n",
    coordinate("shared-actions", "x")
))]
fn rejects_any_other_owner_reference(#[case] content: String) -> Outcome {
    expect("notes.md", &content, Verdict::Rejected)
}

#[rstest]
#[case::markdown("notes.md")]
#[case::rust("lib.rs")]
#[case::python("tool.py")]
#[case::toml("settings.toml")]
fn polices_every_listed_source_type(#[case] file_name: &str) -> Outcome {
    expect(file_name, &format!("{OWNER}/bare\n"), Verdict::Rejected)
}

#[rstest]
fn ignores_unlisted_file_types() -> Outcome {
    expect("data.txt", &format!("{OWNER}/bare\n"), Verdict::Accepted)
}

#[rstest]
#[case::path(concat!("see /da", "ta/scratch\n"))]
#[case::word(concat!("a Concor", "dat mention\n"))]
/// The words are split so this file does not itself contain them.
fn rejects_non_reproducible_provenance_words(#[case] content: &str) -> Outcome {
    expect("notes.md", content, Verdict::Rejected)
}

/// Turn an I/O failure into a failed proptest case.
fn fail(error: &io::Error) -> TestCaseError { TestCaseError::fail(error.to_string()) }

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Every well-formed coordinate is accepted, wherever it sits on the line.
    #[test]
    fn well_formed_coordinates_are_accepted(
        repository in NAME,
        action in NAME,
        prefix in "[A-Za-z ]{0,8}",
    ) {
        let line = format!("{prefix}{}\n", coordinate(&repository, &action));
        prop_assert!(provenance_accepts("notes.md", &line).map_err(|error| fail(&error))?);
    }

    /// A reference that is not a coordinate is rejected, and so is a valid
    /// coordinate that shares its line with one.
    #[test]
    fn stray_references_are_rejected(
        repository in NAME,
        suffix in prop::sample::select(vec!["", "#7", ".git", "/tree/main"]),
        keep_valid_coordinate in any::<bool>(),
    ) {
        let stray = format!("{OWNER}/{repository}{suffix}");
        let line = if keep_valid_coordinate {
            format!("{} {stray}\n", coordinate("shared-actions", "x"))
        } else {
            format!("{stray}\n")
        };
        prop_assert!(!provenance_accepts("notes.md", &line).map_err(|error| fail(&error))?);
    }
}
