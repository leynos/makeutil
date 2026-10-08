//! Contract tests that the Makefile's Markdown formatting wiring meets the estate baseline.
//!
//! `make check-fmt` must run `mdtablefix --check` and `make fmt` `mdtablefix --in-place`, each
//! over the Git-selected Markdown set with all five rewrite flags and its exit status reaching
//! Make. The CI half (the install step and the lint action) is in `markdown_ci_wiring.rs`. The
//! Makefile is read with
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

use super::reading::manifest_dir;

/// Flags the selection and rewrite share: the two that select the files and
/// the five that choose the rewrites. A check without the rewrite flags passes
/// files `make fmt` would still change, and a `fmt` without any of them stops
/// formatting what the check expects.
const SHARED_FLAGS: [&str; 7] = [
    "--git",
    "--include-untracked",
    "--wrap",
    "--renumber",
    "--breaks",
    "--ellipsis",
    "--fences",
];

/// The target to read and the mode flag its mdtablefix invocation must carry.
#[derive(Clone, Copy)]
struct Target {
    name: &'static str,
    mode: &'static str,
}

const CHECK_FMT: Target = Target {
    name: "check-fmt",
    mode: "--check",
};
const FMT: Target = Target {
    name: "fmt",
    mode: "--in-place",
};

impl Target {
    /// Returns every flag the target's invocation must carry.
    fn required(self) -> Vec<&'static str> {
        std::iter::once(self.mode).chain(SHARED_FLAGS).collect()
    }
}

/// The estate variables, as the repository's Makefile spells them.
const VARIABLES: &str = concat!(
    "MDLINT ?= markdownlint-cli2\n",
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

/// Returns the text with any shell comment removed: from the first `#` that starts a word.
/// The shell does not pass what follows to the command.
fn without_comment(text: &str) -> &str {
    let start = text
        .char_indices()
        .find(|&(index, c)| {
            c == '#'
                && text
                    .get(..index)
                    .is_none_or(|before| before.is_empty() || before.ends_with(char::is_whitespace))
        })
        .map_or(text.len(), |(index, _)| index);
    text.get(..start).unwrap_or(text)
}

/// Returns whether one `&&` segment runs mdtablefix with every required flag.
fn is_invocation(segment: &str, target: Target) -> bool {
    let mut words = segment.split_whitespace();
    let is_mdtablefix = words
        .next()
        .is_some_and(|program| program.rsplit('/').next() == Some("mdtablefix"));
    let arguments: Vec<&str> = words.collect();
    is_mdtablefix
        && target
            .required()
            .iter()
            .all(|flag| arguments.contains(flag))
}

/// Returns whether one `&&` segment runs markdownlint-cli2 with `--fix`.
fn is_linter_fix(segment: &str) -> bool {
    let mut words = segment.split_whitespace();
    let is_linter = words
        .next()
        .is_some_and(|program| program.rsplit('/').next() == Some("markdownlint-cli2"));
    is_linter && words.any(|word| word == "--fix")
}

/// Returns whether a recipe's exit status reaches Make and one of its `&&` segments is accepted.
fn recipe_runs(
    recipe: &RecipeFact,
    known: &BTreeMap<&str, &str>,
    accepted: impl Fn(&str) -> bool,
) -> bool {
    // The parser reports the `@`, `+` and `-` prefixes as flags; whether the
    // text keeps them is its own business, so they are dropped before reading.
    let expanded = expand(recipe.text.trim_start_matches(['@', '+', '-', ' ']), known);
    let text = without_comment(&expanded);
    let masks_status = recipe.ignore_errors || text.contains('|') || text.contains(';');
    !masks_status && text.split("&&").any(accepted)
}

/// Returns whether the Makefile's recipe for `target` has a recipe `accepted` approves.
fn target_runs(
    makefile: &str,
    target: Target,
    accepted: impl Fn(&str) -> bool + Copy,
) -> Result<bool, ParseApplicationError> {
    let report = parse(makefile)?;
    let known = variables(&report);
    Ok(report
        .rules
        .iter()
        .filter(|rule| rule.targets.iter().any(|name| name == target.name))
        .flat_map(|rule| rule.recipes.iter())
        .any(|recipe| recipe_runs(recipe, &known, accepted)))
}

/// Returns whether the Makefile's rule for ``target`` runs mdtablefix as the
/// estate standard says.
fn runs_mdtablefix(makefile: &str, target: Target) -> Result<bool, ParseApplicationError> {
    target_runs(makefile, target, |segment| is_invocation(segment, target))
}

/// Returns whether `make fmt` runs `markdownlint-cli2 --fix` with its status reaching Make.
fn runs_linter_fix(makefile: &str) -> Result<bool, ParseApplicationError> {
    target_runs(makefile, FMT, is_linter_fix)
}

/// Returns a minimal Makefile whose `check-fmt` runs one recipe line.
fn makefile(recipe: &str, variables: &str) -> String { makefile_for(CHECK_FMT, recipe, variables) }

/// Returns a minimal Makefile whose ``target`` runs one recipe line.
fn makefile_for(target: Target, recipe: &str, variables: &str) -> String {
    format!("{variables}\n{}: ## A target\n\t{recipe}\n", target.name)
}

#[test]
fn the_repository_makefile_runs_the_check() {
    let text = manifest_dir()
        .and_then(|dir| dir.read_to_string("Makefile"))
        .expect("the Makefile is readable");
    assert!(runs_mdtablefix(&text, CHECK_FMT).expect("the Makefile parses"));
}

#[test]
fn the_repository_makefile_runs_the_rewrite() {
    let text = manifest_dir()
        .and_then(|dir| dir.read_to_string("Makefile"))
        .expect("the Makefile is readable");
    assert!(runs_mdtablefix(&text, FMT).expect("the Makefile parses"));
}

#[rstest]
#[case::no_check("$(MDTABLEFIX) $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)")]
#[case::no_select("$(MDTABLEFIX) --check $(MDTABLEFIX_RULES)")]
#[case::no_rules("$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT)")]
#[case::ignored("-$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)")]
#[case::or_true("$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES) || true")]
#[case::sequenced("$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES); true")]
#[case::echoed("echo $(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)")]
#[case::flags_in_a_comment(
    "mdtablefix --check --git --include-untracked # --wrap --renumber --breaks --ellipsis --fences"
)]
#[case::variable_in_a_comment("$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) # $(MDTABLEFIX_RULES)")]
fn a_weakened_check_is_refused(#[case] recipe: &str) {
    assert!(!runs_mdtablefix(&makefile(recipe, VARIABLES), CHECK_FMT).expect("the fixture parses"));
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
#[case::trailing_comment(
    "$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES) # the estate recipe",
    VARIABLES
)]
#[case::qualified(
    "/usr/local/bin/mdtablefix --check --git --include-untracked --wrap --renumber --breaks \
     --ellipsis --fences",
    ""
)]
fn a_legitimate_check_is_accepted(#[case] recipe: &str, #[case] variables: &str) {
    assert!(runs_mdtablefix(&makefile(recipe, variables), CHECK_FMT).expect("the fixture parses"));
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
    let flags: Vec<&str> = CHECK_FMT
        .required()
        .iter()
        .copied()
        .filter(|f| *f != dropped)
        .collect();
    let recipe = format!("mdtablefix {}", flags.join(" "));
    assert!(!runs_mdtablefix(&makefile(&recipe, ""), CHECK_FMT).expect("the fixture parses"));
}

#[rstest]
#[case::mode("--in-place")]
#[case::git("--git")]
#[case::untracked("--include-untracked")]
#[case::wrap("--wrap")]
#[case::renumber("--renumber")]
#[case::breaks("--breaks")]
#[case::ellipsis("--ellipsis")]
#[case::fences("--fences")]
fn fmt_dropping_any_single_required_flag_is_refused(#[case] dropped: &str) {
    let flags: Vec<&str> = FMT
        .required()
        .into_iter()
        .filter(|f| *f != dropped)
        .collect();
    let recipe = format!("mdtablefix {}", flags.join(" "));
    let text = makefile_for(FMT, &recipe, "");
    assert!(!runs_mdtablefix(&text, FMT).expect("the fixture parses"));
}

#[rstest]
#[case::reference(
    "$(MDTABLEFIX) --in-place $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)",
    true
)]
#[case::check_instead(
    "$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)",
    false
)]
#[case::ignored(
    "-$(MDTABLEFIX) --in-place $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)",
    false
)]
#[case::or_true(
    "$(MDTABLEFIX) --in-place $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES) || true",
    false
)]
fn fmt_is_held_to_the_rewrite_contract(#[case] recipe: &str, #[case] accepted: bool) {
    let text = makefile_for(FMT, recipe, VARIABLES);
    assert_eq!(
        runs_mdtablefix(&text, FMT).expect("the fixture parses"),
        accepted
    );
}

#[test]
fn the_repository_makefile_runs_the_linter_fix() {
    let text = manifest_dir()
        .and_then(|dir| dir.read_to_string("Makefile"))
        .expect("the Makefile is readable");
    assert!(runs_linter_fix(&text).expect("the Makefile parses"));
}

#[rstest]
#[case::reference("$(MDLINT) --fix \"**/*.md\"", true)]
#[case::literal("markdownlint-cli2 --fix \"**/*.md\"", true)]
#[case::chained("cargo fmt && $(MDLINT) --fix \"**/*.md\"", true)]
#[case::no_fix("$(MDLINT) \"**/*.md\"", false)]
#[case::ignored("-$(MDLINT) --fix \"**/*.md\"", false)]
#[case::or_true("$(MDLINT) --fix \"**/*.md\" || true", false)]
#[case::sequenced("$(MDLINT) --fix \"**/*.md\"; true", false)]
#[case::echoed("echo $(MDLINT) --fix \"**/*.md\"", false)]
#[case::fix_in_a_comment("$(MDLINT) \"**/*.md\" # --fix", false)]
fn fmt_is_held_to_the_linter_fix_contract(#[case] recipe: &str, #[case] accepted: bool) {
    let text = makefile_for(FMT, recipe, VARIABLES);
    assert_eq!(
        runs_linter_fix(&text).expect("the fixture parses"),
        accepted
    );
}
