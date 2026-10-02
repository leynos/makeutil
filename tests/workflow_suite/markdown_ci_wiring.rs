//! Contract tests that the CI half of the Markdown formatting wiring meets the estate baseline.
//!
//! The job that runs `make check-fmt` must install mdtablefix in an earlier step, and every
//! markdownlint-cli2-action step must lint `**/*.md`. Workflow text is read line by line, so a
//! commented-out installer, a folded `uses: >-` installer and a chained `make check-fmt && ...`
//! are each exercised against the repository's own workflows and weakened fixtures.

use super::reading::{Command, workflows};

/// The shared action that installs mdtablefix.
const INSTALL_ACTION: &str = "leynos/shared-actions/.github/actions/install-mdtablefix@";

/// The upstream Markdown lint action.
const LINT_ACTION: &str = "DavidAnson/markdownlint-cli2-action@";

/// Returns whether the line opens a job: a key one level under `jobs:`.
fn is_job_key(line: &str) -> bool {
    indent(line) == 2 && line.trim_end().ends_with(':') && !line.trim_start().starts_with('-')
}

/// Returns whether `make check-fmt` at line `index` has no earlier
/// install step in its job. The action may be named on the `uses:` line or,
/// folded with `>-`, on the line after it, so the job's text is searched.
fn checks_before_install(lines: &[&str], index: usize) -> bool {
    let head = lines.get(..index).unwrap_or_default();
    let job_start = head.iter().rposition(|line| is_job_key(line)).unwrap_or(0);
    let job_head = head.get(job_start..).unwrap_or_default();
    !job_head.iter().any(|line| installs_mdtablefix(line))
}

/// Returns whether a line names the install action outside a comment, so a
/// commented-out installer does not satisfy the contract.
fn installs_mdtablefix(line: &str) -> bool {
    !line.trim_start().starts_with('#') && line.contains(INSTALL_ACTION)
}

/// Returns whether any `&&`, `;`, `|` or `&` segment of the line is exactly
/// `make check-fmt`, so `make check-fmt && make test` is still seen.
fn runs_check_fmt(line: &str) -> bool {
    Command::from_line(line)
        .text()
        .split([';', '|', '&'])
        .any(|segment| segment.trim() == "make check-fmt")
}

/// Returns each `workflow:line` where `make check-fmt` runs uninstalled.
fn uninstalled_check_fmt(found: &[(String, String)]) -> Vec<String> {
    let mut missing = Vec::new();
    for (name, text) in found {
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if runs_check_fmt(line) && checks_before_install(&lines, index) {
                missing.push(format!("{name}:{}", index + 1));
            }
        }
    }
    missing
}

/// Returns the indentation width of a line.
fn indent(line: &str) -> usize { line.len() - line.trim_start().len() }

/// Returns the lines after `start` that belong to the same step: those
/// indented deeper than the step's `- ` item.
fn step_tail<'a>(lines: &[&'a str], start: usize) -> Vec<&'a str> {
    let head = lines.get(..=start).unwrap_or_default();
    let depth = head
        .iter()
        .rev()
        .find(|line| line.trim_start().starts_with("- "))
        .map_or(0, |line| indent(line));
    lines
        .iter()
        .skip(start + 1)
        .take_while(|line| line.trim().is_empty() || indent(line) > depth)
        .copied()
        .collect()
}

/// Returns `(action steps, steps not linting **/*.md)` across the workflows.
fn lint_action_globs(found: &[(String, String)]) -> (usize, usize) {
    let mut steps = 0;
    let mut narrowed = 0;
    for (_, text) in found {
        let lines: Vec<&str> = text.lines().collect();
        for (index, _) in lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains("uses:") && line.contains(LINT_ACTION))
        {
            steps += 1;
            let lints_everything = step_tail(&lines, index)
                .iter()
                .any(|line| line.trim() == "globs: '**/*.md'");
            narrowed += usize::from(!lints_everything);
        }
    }
    (steps, narrowed)
}

#[test]
fn the_repository_workflows_install_and_lint() {
    let found = workflows().expect("the workflows are readable");
    assert_eq!(uninstalled_check_fmt(&found), Vec::<String>::new());
    let (steps, narrowed) = lint_action_globs(&found);
    assert!(steps > 0, "no step runs the markdownlint-cli2-action");
    assert_eq!(
        narrowed, 0,
        "a markdownlint-cli2-action step lints less than **/*.md"
    );
}

#[test]
fn an_install_after_check_fmt_is_refused() {
    let text = concat!(
        "jobs:\n  build-test:\n    steps:\n",
        "      - run: make check-fmt\n",
        "      - uses: leynos/shared-actions/.github/actions/install-mdtablefix@abc\n",
    );
    let found = vec![("ci.yml".to_owned(), text.to_owned())];
    assert_eq!(uninstalled_check_fmt(&found), vec!["ci.yml:4".to_owned()]);
}

#[test]
fn narrowed_lint_globs_are_refused() {
    let text = concat!(
        "jobs:\n  lint:\n    steps:\n",
        "      - uses: DavidAnson/markdownlint-cli2-action@abc\n",
        "        with:\n          globs: 'docs/**/*.md'\n",
    );
    let found = vec![("ci.yml".to_owned(), text.to_owned())];
    assert_eq!(lint_action_globs(&found), (1, 1));
}

#[test]
fn a_commented_out_install_does_not_count() {
    let text = concat!(
        "jobs:\n  build-test:\n    steps:\n",
        "      # - uses: leynos/shared-actions/.github/actions/install-mdtablefix@abc\n",
        "      - run: make check-fmt\n",
    );
    let found = vec![("ci.yml".to_owned(), text.to_owned())];
    assert_eq!(uninstalled_check_fmt(&found), vec!["ci.yml:5".to_owned()]);
}

#[test]
fn a_chained_check_fmt_is_still_seen() {
    let text = concat!(
        "jobs:\n  build-test:\n    steps:\n",
        "      - run: make check-fmt && make test\n",
    );
    let found = vec![("ci.yml".to_owned(), text.to_owned())];
    assert_eq!(uninstalled_check_fmt(&found), vec!["ci.yml:4".to_owned()]);
}

#[test]
fn a_folded_install_before_the_check_is_accepted() {
    let text = concat!(
        "jobs:\n  build-test:\n    steps:\n",
        "      - name: Install mdtablefix\n",
        "        uses: >-\n",
        "          leynos/shared-actions/.github/actions/install-mdtablefix@abc\n",
        "      - run: make check-fmt\n",
    );
    let found = vec![("ci.yml".to_owned(), text.to_owned())];
    assert_eq!(uninstalled_check_fmt(&found), Vec::<String>::new());
}
