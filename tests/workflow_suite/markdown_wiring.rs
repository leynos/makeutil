//! Contract tests that the Markdown formatting wiring meets the estate baseline.
//!
//! `make check-fmt` must run `mdtablefix --check` over the Git-selected
//! Markdown set with all five rewrite flags and its exit status reaching Make; the CI job that runs
//! `make check-fmt` must install mdtablefix in an earlier step; and every
//! markdownlint-cli2-action step must lint `**/*.md`. The Makefile is read with
//! makeutil's own parser, so the recipe, its prefixes and the variables it
//! references are the parser's facts rather than a second reading of the text.
//! Each clause is exercised against weakened and legitimate fixtures as well
//! as the repository's own files.

use std::collections::BTreeMap;

use makeutil::{
    ParseApplicationError,
    adapters::MakefileLosslessParser,
    domain::{ParseReport, RecipeFact},
    parse_source,
};
use rstest::rstest;

use super::reading::{Command, manifest_dir, workflows};

/// Flags `check-fmt` must pass to mdtablefix, in any order: the mode, the two
/// that select the files and the five that choose the rewrites. A check
/// without the rewrite flags passes files `make fmt` would still change.
const REQUIRED_FLAGS: [&str; 8] = [
    "--check",
    "--git",
    "--include-untracked",
    "--wrap",
    "--renumber",
    "--breaks",
    "--ellipsis",
    "--fences",
];

/// The shared action that installs mdtablefix.
const INSTALL_ACTION: &str = "leynos/shared-actions/.github/actions/install-mdtablefix@";

/// The upstream Markdown lint action.
const LINT_ACTION: &str = "DavidAnson/markdownlint-cli2-action@";

/// The estate variables, as the repository's Makefile spells them.
const VARIABLES: &str = concat!(
    "MDTABLEFIX ?= mdtablefix\n",
    "MDTABLEFIX_SELECT = --git --include-untracked\n",
    "MDTABLEFIX_RULES = --wrap --renumber --breaks --ellipsis --fences\n",
);

/// Parses Makefile text with makeutil's parser.
fn parse(text: &str) -> Result<ParseReport, ParseApplicationError> {
    parse_source(text.as_bytes(), "Makefile", &MakefileLosslessParser)
}

/// Returns each variable with exactly one unconditional, non-`define`
/// assignment, so a reference to anything else stays unexpanded.
fn variables(report: &ParseReport) -> BTreeMap<&str, &str> {
    let mut seen: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for fact in report
        .variables
        .iter()
        .filter(|fact| fact.conditions.is_empty() && !fact.define_block)
    {
        seen.entry(fact.name.as_str())
            .or_default()
            .push(fact.raw_value.trim());
    }
    seen.into_iter()
        .filter_map(|(name, values)| match values.as_slice() {
            [only] => Some((name, *only)),
            _ => None,
        })
        .collect()
}

/// Expands `$(NAME)` references, three passes deep.
fn expand(text: &str, known: &BTreeMap<&str, &str>) -> String {
    let mut expanded = text.to_owned();
    for _ in 0..3 {
        for (name, value) in known {
            expanded = expanded.replace(&format!("$({name})"), value);
        }
    }
    expanded
}

/// Returns whether one `&&` segment runs mdtablefix with every required flag.
fn is_check_invocation(segment: &str) -> bool {
    let mut words = segment.split_whitespace();
    let is_mdtablefix = words
        .next()
        .is_some_and(|program| program.rsplit('/').next() == Some("mdtablefix"));
    let arguments: Vec<&str> = words.collect();
    is_mdtablefix && REQUIRED_FLAGS.iter().all(|flag| arguments.contains(flag))
}

/// Returns whether a recipe's exit status reaches Make and it runs the check.
fn recipe_runs_check(recipe: &RecipeFact, known: &BTreeMap<&str, &str>) -> bool {
    // The parser reports the `@`, `+` and `-` prefixes as flags; whether the
    // text keeps them is its own business, so they are dropped before reading.
    let text = expand(recipe.text.trim_start_matches(['@', '+', '-', ' ']), known);
    let masks_status = recipe.ignore_errors || text.contains('|') || text.contains(';');
    !masks_status && text.split("&&").any(is_check_invocation)
}

/// Returns whether the Makefile's `check-fmt` rule runs the mdtablefix check.
fn runs_mdtablefix_check(makefile: &str) -> Result<bool, ParseApplicationError> {
    let report = parse(makefile)?;
    let known = variables(&report);
    Ok(report
        .rules
        .iter()
        .filter(|rule| rule.targets.iter().any(|target| target == "check-fmt"))
        .flat_map(|rule| rule.recipes.iter())
        .any(|recipe| recipe_runs_check(recipe, &known)))
}

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

/// Returns a minimal Makefile whose `check-fmt` runs one recipe line.
fn makefile(recipe: &str, variables: &str) -> String {
    format!("{variables}\ncheck-fmt: ## Verify formatting\n\t{recipe}\n")
}

#[test]
fn the_repository_makefile_runs_the_check() {
    let text = manifest_dir()
        .and_then(|dir| dir.read_to_string("Makefile"))
        .expect("the Makefile is readable");
    assert!(runs_mdtablefix_check(&text).expect("the Makefile parses"));
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

#[rstest]
#[case::no_check("$(MDTABLEFIX) $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)")]
#[case::no_select("$(MDTABLEFIX) --check $(MDTABLEFIX_RULES)")]
#[case::no_rules("$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT)")]
#[case::ignored("-$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)")]
#[case::or_true("$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES) || true")]
#[case::sequenced("$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES); true")]
#[case::echoed("echo $(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)")]
fn a_weakened_check_is_refused(#[case] recipe: &str) {
    assert!(!runs_mdtablefix_check(&makefile(recipe, VARIABLES)).expect("the fixture parses"));
}

#[rstest]
#[case::reference(
    "$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)",
    VARIABLES
)]
#[case::reordered_literal(
    "mdtablefix --fences --include-untracked --check --ellipsis --git --wrap --breaks --renumber",
    ""
)]
#[case::silenced(
    "@$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)",
    VARIABLES
)]
#[case::chained(
    "cargo fmt --check && mdtablefix --check --git --include-untracked --wrap --renumber --breaks \
     --ellipsis --fences",
    ""
)]
#[case::qualified(
    "/usr/local/bin/mdtablefix --check --git --include-untracked --wrap --renumber --breaks \
     --ellipsis --fences",
    ""
)]
fn a_legitimate_check_is_accepted(#[case] recipe: &str, #[case] variables: &str) {
    assert!(runs_mdtablefix_check(&makefile(recipe, variables)).expect("the fixture parses"));
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

#[rstest]
#[case::mode("--check")]
#[case::git("--git")]
#[case::untracked("--include-untracked")]
#[case::wrap("--wrap")]
#[case::renumber("--renumber")]
#[case::breaks("--breaks")]
#[case::ellipsis("--ellipsis")]
#[case::fences("--fences")]
fn dropping_any_single_required_flag_is_refused(#[case] dropped: &str) {
    let flags: Vec<&str> = REQUIRED_FLAGS
        .iter()
        .copied()
        .filter(|f| *f != dropped)
        .collect();
    let recipe = format!("mdtablefix {}", flags.join(" "));
    assert!(!runs_mdtablefix_check(&makefile(&recipe, "")).expect("the fixture parses"));
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
