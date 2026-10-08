//! Contract tests for the environment of the build steps that sit outside the
//! Makefile's development recipes.
//!
//! The build standard's own contract (`build_standard_contract.rs`) holds the
//! development recipes and the `setup-rust` steps. Four builds assign their own
//! environment instead, and an assigned `RUSTFLAGS` replaces every `rustflags`
//! table in `.cargo/config.toml`, so each is pinned here to the value it needs:
//!
//! - the coverage recipe and the CI coverage step link with `lld` through `clang`, because LLVM
//!   coverage tools expect LLVM-compatible linker behaviour, and take neither the frontend flag nor
//!   mold;
//! - the doctest step restates the flags `make test` gives its doctest line;
//! - the release build step replaces the repository's `rustflags`, whose Linux entry links with
//!   mold, with the borrow-checker flag alone, so a static musl binary links with the runner's
//!   default.
//!
//! Workflow steps are read as text and judged by their own `env:` block, so a
//! sibling step's environment, a comment and an inline comment neither supply nor
//! hide a value. The recipe is read from what `make -n coverage` prints.
//! Fixtures come first, so no rule passes by finding nothing.

use std::process::Command;

/// A workflow file, with its text.
#[derive(Clone, Copy)]
struct Workflow {
    file: &'static str,
    text: &'static str,
}

/// What one step's `env:` block must hold.
struct Expected {
    workflow: Workflow,
    step: &'static str,
    /// Variables that must be present with exactly this value, so a standard flag added to
    /// `RUSTFLAGS` is a mismatch like any other.
    env: &'static [(&'static str, &'static str)],
}

const CI: Workflow = Workflow {
    file: "ci.yml",
    text: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/.github/workflows/ci.yml"
    )),
};

const RELEASE: Workflow = Workflow {
    file: "release.yml",
    text: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/.github/workflows/release.yml"
    )),
};

/// The frontend flag and the linker flag the measurement goes without.
const STANDARD_FLAGS: &[&str] = &["-Zthreads=8", "-Clink-arg=-fuse-ld=mold"];

const EXPECTED: &[Expected] = &[
    Expected {
        workflow: CI,
        step: "Test and Measure Coverage",
        env: &[
            ("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER", "clang"),
            ("RUSTFLAGS", "-Zpolonius=next -C link-arg=-fuse-ld=lld"),
            ("CFLAGS", "-fuse-ld=lld"),
            ("LDFLAGS", "-fuse-ld=lld"),
        ],
    },
    Expected {
        workflow: CI,
        step: "Run doctests",
        env: &[(
            "RUSTFLAGS",
            "-Zpolonius=next -D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold",
        )],
    },
    Expected {
        workflow: RELEASE,
        step: "Build release binary",
        env: &[("RUSTFLAGS", "-Zpolonius=next")],
    },
];

/// The environment the coverage recipe's command must assign before `cargo`.
const COVERAGE_RECIPE: &[(&str, &str)] = &[
    ("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER", "clang"),
    ("CARGO_PROFILE_DEV_CODEGEN_BACKEND", "llvm"),
    ("CFLAGS", "-fuse-ld=lld"),
    ("LDFLAGS", "-fuse-ld=lld"),
];

/// Returns the number of leading spaces on a line.
fn indent(line: &str) -> usize { line.len() - line.trim_start().len() }

/// Returns whether a line carries nothing a step is judged by.
fn is_blank_or_comment(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

/// Returns the lines of every step with the given `name:`: from its list item to
/// the line before the next item at the same indentation or a dedent.
fn steps_named<'a>(text: &'a str, name: &str) -> Vec<Vec<&'a str>> {
    let lines: Vec<&str> = text.lines().collect();
    let opens = |line: &str| {
        let item = line.trim_start().strip_prefix("- ").unwrap_or_default();
        item.strip_prefix("name:")
            .is_some_and(|rest| rest.trim().trim_matches(['"', '\'']) == name)
    };
    let mut found = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        if !opens(line) {
            continue;
        }
        let item_indent = indent(line);
        let end = lines
            .iter()
            .enumerate()
            .skip(at + 1)
            .find(|(_, next)| {
                !is_blank_or_comment(next)
                    && (indent(next) < item_indent
                        || (indent(next) == item_indent && next.trim_start().starts_with("- ")))
            })
            .map_or(lines.len(), |(index, _)| index);
        found.push(lines.get(at..end).unwrap_or_default().to_vec());
    }
    found
}

/// Returns a value without any inline YAML comment or quotes.
fn plain(value: &str) -> &str {
    value
        .split(" #")
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches(['"', '\''])
}

/// Returns the entries of a step's own `env:` block.
fn env_of(step: &[&str]) -> Vec<(String, String)> {
    let Some(at) = step.iter().position(|line| line.trim() == "env:") else {
        return Vec::new();
    };
    let env_indent = step.get(at).map_or(0, |line| indent(line));
    step.iter()
        .skip(at + 1)
        .take_while(|line| is_blank_or_comment(line) || indent(line) > env_indent)
        .filter(|line| !is_blank_or_comment(line))
        .filter_map(|line| line.trim().split_once(':'))
        .map(|(key, value)| (key.trim().to_owned(), plain(value).to_owned()))
        .collect()
}

/// Returns the complaints about one step expectation against a workflow text.
fn step_problems(file: &str, text: &str, expected: &Expected) -> Vec<String> {
    let named = steps_named(text, expected.step);
    let [step] = named.as_slice() else {
        return vec![format!(
            "{file}: expected one step named {:?}, found {}",
            expected.step,
            named.len()
        )];
    };
    let env = env_of(step);
    let value_of = |key: &str| {
        env.iter()
            .rev()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    };
    let mut problems = Vec::new();
    for (key, wanted) in expected.env {
        match value_of(key) {
            Some(found) if found == *wanted => {}
            found => problems.push(format!(
                "{file}: step {:?} has {key}={found:?}, not {wanted:?}",
                expected.step
            )),
        }
    }
    problems
}

/// Splits the text after an `=` into a double-quoted or bare value and the rest.
fn split_value(after: &str) -> (&str, &str) {
    after.strip_prefix('"').map_or_else(
        || after.split_once(char::is_whitespace).unwrap_or((after, "")),
        |quoted| quoted.split_once('"').unwrap_or((quoted, "")),
    )
}

/// Returns the leading `NAME=value` assignments of a command line, where a
/// value is a double-quoted string or a bare word.
///
/// ```text
/// CFLAGS="-fuse-ld=lld" LDFLAGS=x cargo llvm-cov  ->  [(CFLAGS, -fuse-ld=lld), (LDFLAGS, x)]
/// ```
fn leading_assignments(command: &str) -> Vec<(String, String)> {
    let mut rest = command.trim_start();
    let mut found = Vec::new();
    while let Some((name, after)) = rest.split_once('=') {
        let is_name = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if !is_name {
            break;
        }
        let (value, remainder) = split_value(after);
        found.push((name.to_owned(), value.to_owned()));
        rest = remainder.trim_start();
    }
    found
}

/// Returns the complaints about what `make -n coverage` printed: exactly one
/// `cargo llvm-cov` command, assigning the required environment before it, with
/// `RUSTFLAGS` linking through `lld` and naming neither standard flag.
fn recipe_problems(printed: &str) -> Vec<String> {
    let joined = printed.replace("\\\n", " ");
    let runs: Vec<&str> = joined
        .lines()
        .filter(|line| line.contains(" llvm-cov") && line.contains("cargo"))
        .collect();
    let [command] = runs.as_slice() else {
        return vec![format!(
            "expected one `cargo llvm-cov` command, found {}",
            runs.len()
        )];
    };
    let assigned = leading_assignments(command);
    let value_of = |key: &str| {
        assigned
            .iter()
            .rev()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    };
    let mut problems = Vec::new();
    for (key, wanted) in COVERAGE_RECIPE {
        match value_of(key) {
            Some(found) if found == *wanted => {}
            found => problems.push(format!(
                "coverage recipe has {key}={found:?}, not {wanted:?}"
            )),
        }
    }
    let flags = value_of("RUSTFLAGS").unwrap_or_default();
    if !flags.contains("-C link-arg=-fuse-ld=lld") {
        problems.push(format!(
            "coverage RUSTFLAGS {flags:?} does not link with lld"
        ));
    }
    problems.extend(
        STANDARD_FLAGS
            .iter()
            .filter(|word| flags.split_whitespace().any(|flag| flag == **word))
            .map(|word| format!("coverage RUSTFLAGS names {word}")),
    );
    problems
}

/// Returns what `make -n coverage` prints.
fn printed_coverage() -> Result<String, String> {
    let output = Command::new("make")
        .args(["-n", "-B", "coverage"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .map_err(|error| format!("running make: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "`make -n coverage` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// Turns a list of complaints into a test result.
fn none_of(problems: &[String]) -> Result<(), String> {
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("{problems:#?}"))
    }
}

const COVERAGE_STEP: Expected = Expected {
    workflow: CI,
    step: "Cover",
    env: &[
        ("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER", "clang"),
        ("RUSTFLAGS", "-Zpolonius=next -C link-arg=-fuse-ld=lld"),
    ],
};

const STEP_OK: &str = "\
    steps:
      - name: Cover
        env:
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: clang
          # the measurement links with lld
          RUSTFLAGS: -Zpolonius=next -C link-arg=-fuse-ld=lld # no mold
        run: cargo llvm-cov
";

const STEP_WRONG_VALUE: &str = "\
    steps:
      - name: Cover
        env:
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: gcc
          RUSTFLAGS: -Zpolonius=next -C link-arg=-fuse-ld=lld
";

const STEP_MISSING_KEY: &str = "\
    steps:
      - name: Cover
        env:
          RUSTFLAGS: -Zpolonius=next -C link-arg=-fuse-ld=lld
";

const STEP_WITH_MOLD: &str = "\
    steps:
      - name: Cover
        env:
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: clang
          RUSTFLAGS: -Zpolonius=next -C link-arg=-fuse-ld=lld -Clink-arg=-fuse-ld=mold
";

const STEP_WITH_FRONTEND: &str = "\
    steps:
      - name: Cover
        env:
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: clang
          RUSTFLAGS: -Zpolonius=next -C link-arg=-fuse-ld=lld -Zthreads=8
";

const STEP_BORROWING_A_SIBLING: &str = "\
    steps:
      - name: Cover
        run: cargo llvm-cov
      - name: Sibling
        env:
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: clang
          RUSTFLAGS: -Zpolonius=next -C link-arg=-fuse-ld=lld
";

const STEP_TWICE: &str = "\
    steps:
      - name: Cover
        env:
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: clang
          RUSTFLAGS: -Zpolonius=next -C link-arg=-fuse-ld=lld
      - name: Cover
        run: true
";

const STEP_COMMENTED_ONLY: &str = "\
    steps:
      - name: Cover
        env:
          # CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: clang
          RUSTFLAGS: -Zpolonius=next -C link-arg=-fuse-ld=lld
";

const RECIPE_OK: &str = "echo \"coverage linker flags: -fuse-ld=lld\"
CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=clang \\
\tCARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm \\
\tRUSTFLAGS=\"-D warnings -Zpolonius=next -C link-arg=-fuse-ld=lld\" \\
\tCFLAGS=\"-fuse-ld=lld\" \\
\tLDFLAGS=\"-fuse-ld=lld\" \\
\tcargo llvm-cov --lcov --output-path lcov.info
";

#[test]
fn the_real_steps_hold_their_environment() -> Result<(), String> {
    let problems: Vec<String> = EXPECTED
        .iter()
        .flat_map(|expected| {
            step_problems(expected.workflow.file, expected.workflow.text, expected)
        })
        .collect();
    none_of(&problems)
}

#[test]
fn the_real_coverage_recipe_holds_its_environment() -> Result<(), String> {
    none_of(&recipe_problems(&printed_coverage()?))
}

#[test]
fn a_step_that_holds_its_environment_is_accepted() -> Result<(), String> {
    none_of(&step_problems("fixture.yml", STEP_OK, &COVERAGE_STEP))
}

#[test]
fn a_step_that_loses_or_changes_a_value_is_refused() {
    for (label, text) in [
        ("a wrong value", STEP_WRONG_VALUE),
        ("a missing key", STEP_MISSING_KEY),
        ("a commented-out key", STEP_COMMENTED_ONLY),
    ] {
        assert_eq!(
            step_problems("fixture.yml", text, &COVERAGE_STEP).len(),
            1,
            "{label}"
        );
    }
}

#[test]
fn a_step_that_names_a_standard_flag_is_refused() {
    for (label, text) in [
        ("mold", STEP_WITH_MOLD),
        ("the frontend", STEP_WITH_FRONTEND),
    ] {
        assert_eq!(
            step_problems("fixture.yml", text, &COVERAGE_STEP).len(),
            1,
            "{label}"
        );
    }
}

#[test]
fn a_step_cannot_borrow_a_siblings_environment() {
    assert_eq!(
        step_problems("fixture.yml", STEP_BORROWING_A_SIBLING, &COVERAGE_STEP).len(),
        2
    );
}

#[test]
fn a_step_that_is_absent_or_repeated_proves_nothing() {
    assert_eq!(
        step_problems("fixture.yml", "steps: []\n", &COVERAGE_STEP).len(),
        1
    );
    assert_eq!(
        step_problems("fixture.yml", STEP_TWICE, &COVERAGE_STEP).len(),
        1
    );
}

#[test]
fn a_recipe_that_holds_its_environment_is_accepted() -> Result<(), String> {
    none_of(&recipe_problems(RECIPE_OK))
}

#[test]
fn a_recipe_that_loses_or_changes_a_value_is_refused() {
    let lacking = |needle: &str| RECIPE_OK.replace(needle, "");
    for (label, printed) in [
        (
            "no linker",
            lacking("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=clang \\\n"),
        ),
        (
            "no backend",
            lacking("\tCARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm \\\n"),
        ),
        ("no CFLAGS", lacking("\tCFLAGS=\"-fuse-ld=lld\" \\\n")),
        ("no LDFLAGS", lacking("\tLDFLAGS=\"-fuse-ld=lld\" \\\n")),
        ("another backend", RECIPE_OK.replace("=llvm", "=cranelift")),
        ("another linker", RECIPE_OK.replace("=clang", "=gcc")),
        (
            "a different CFLAGS",
            RECIPE_OK.replace("CFLAGS=\"-fuse-ld=lld\"", "CFLAGS=\"-fuse-ld=gold\""),
        ),
        (
            "lld dropped from RUSTFLAGS",
            RECIPE_OK.replace(" -C link-arg=-fuse-ld=lld\"", "\""),
        ),
        (
            "mold named",
            RECIPE_OK.replace("-D warnings", "-D warnings -Clink-arg=-fuse-ld=mold"),
        ),
        (
            "the frontend named",
            RECIPE_OK.replace("-D warnings", "-D warnings -Zthreads=8"),
        ),
        ("no command", "echo coverage\n".to_owned()),
    ] {
        assert!(!recipe_problems(&printed).is_empty(), "{label}");
    }
}
