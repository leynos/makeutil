//! Contract tests that the CI half of the Markdown formatting wiring meets the estate baseline.
//!
//! The job that runs `make check-fmt` must install mdtablefix, at 0.6.1 or later, in an earlier
//! step, and every markdownlint-cli2-action step must lint `**/*.md` through its own `with:`
//! input. Jobs and steps are split by the structure of the indentation, whatever its width, so a
//! commented-out installer, a folded `uses: >-` installer, a chained `make check-fmt && ...`, a
//! four-space workflow and a `globs` key under `env:` are each exercised against the
//! repository's own workflows and weakened fixtures. `.markdownlint-cli2.jsonc` must keep the
//! estate's canonical rule settings, which the lint action and `make fmt` both read.

use serde_json::{Value, json};

use super::reading::{Command, Job, Step, Workflow, manifest_dir, workflows};

/// The shared action that installs mdtablefix.
const INSTALL_ACTION: &str = "leynos/shared-actions/.github/actions/install-mdtablefix@";

/// The upstream Markdown lint action.
const LINT_ACTION: &str = "DavidAnson/markdownlint-cli2-action@";

/// The first mdtablefix release that supports `--git --include-untracked` and the rewrites.
const MINIMUM_VERSION: [u32; 3] = [0, 6, 1];

/// Returns whether any `&&`, `;`, `|` or `&` segment of the line is exactly
/// `make check-fmt`, so `make check-fmt && make test` is still seen.
fn runs_check_fmt(line: &str) -> bool {
    Command::from_line(line)
        .text()
        .split([';', '|', '&'])
        .any(|segment| segment.trim() == "make check-fmt")
}

/// Returns whether a step installs mdtablefix.
fn installs_mdtablefix(step: &Step<'_>) -> bool { step.mentions(INSTALL_ACTION) }

/// Returns whether the job runs `make check-fmt` before any step installs mdtablefix.
fn checks_before_install(job: &Job<'_>) -> bool {
    let mut installed = false;
    for step in job.steps() {
        if !installed && step.commands().any(|c| runs_check_fmt(c.text())) {
            return true;
        }
        installed |= installs_mdtablefix(&step);
    }
    false
}

/// Returns each `workflow:job` whose `make check-fmt` step has no earlier
/// install step in the same job.
fn uninstalled_check_fmt(found: &[(String, String)]) -> Vec<String> {
    let mut missing = Vec::new();
    for (name, text) in found {
        for job in Workflow(text)
            .jobs()
            .iter()
            .filter(|j| checks_before_install(j))
        {
            missing.push(format!("{name}:{}", job.name));
        }
    }
    missing
}

/// Parses `0.6.1`-style text into its three numbers.
fn version(text: &str) -> Option<[u32; 3]> {
    let mut parts = text.split('.').map(|part| part.parse::<u32>().ok());
    let found = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(found)
}

/// Returns whether a step pins a version at or above the minimum.
fn pins_the_minimum(step: &Step<'_>) -> bool {
    step.with_value("version")
        .and_then(|value| version(&value))
        .is_some_and(|number| number >= MINIMUM_VERSION)
}

/// Returns whether the job has an installer step that pins no version, or one below the minimum.
fn has_under_pinned_installer(job: &Job<'_>) -> bool {
    job.steps()
        .iter()
        .any(|step| installs_mdtablefix(step) && !pins_the_minimum(step))
}

/// Returns each `workflow:job` whose install step pins no version, or one below the minimum.
fn under_pinned_installers(found: &[(String, String)]) -> Vec<String> {
    let mut weak = Vec::new();
    for (name, text) in found {
        for job in Workflow(text)
            .jobs()
            .iter()
            .filter(|j| has_under_pinned_installer(j))
        {
            weak.push(format!("{name}:{}", job.name));
        }
    }
    weak
}

/// Returns `(action steps, steps whose `with.globs` is not `**/*.md`)` across the workflows.
fn lint_action_globs(found: &[(String, String)]) -> (usize, usize) {
    let mut steps = 0;
    let mut narrowed = 0;
    for (_, text) in found {
        for job in Workflow(text).jobs() {
            for step in job.steps().iter().filter(|s| s.uses(LINT_ACTION)) {
                steps += 1;
                narrowed += usize::from(step.with_value("globs").as_deref() != Some("**/*.md"));
            }
        }
    }
    (steps, narrowed)
}

/// Wraps one workflow's text as the readers take it.
fn one(text: &str) -> Vec<(String, String)> { vec![("ci.yml".to_owned(), text.to_owned())] }

#[test]
fn the_repository_workflows_install_and_lint() {
    let found = workflows().expect("the workflows are readable");
    assert_eq!(uninstalled_check_fmt(&found), Vec::<String>::new());
    assert_eq!(under_pinned_installers(&found), Vec::<String>::new());
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
    assert_eq!(
        uninstalled_check_fmt(&one(text)),
        vec!["ci.yml:build-test".to_owned()]
    );
}

#[test]
fn an_installer_in_an_earlier_four_space_job_does_not_cover_a_later_job() {
    let text = concat!(
        "jobs:\n",
        "    first:\n        steps:\n",
        "            - uses: leynos/shared-actions/.github/actions/install-mdtablefix@abc\n",
        "              with:\n                  version: '0.6.1'\n",
        "    second:\n        steps:\n",
        "            - run: make check-fmt\n",
    );
    assert_eq!(
        uninstalled_check_fmt(&one(text)),
        vec!["ci.yml:second".to_owned()]
    );
}

#[test]
fn a_commented_out_install_does_not_count() {
    let text = concat!(
        "jobs:\n  build-test:\n    steps:\n",
        "      # - uses: leynos/shared-actions/.github/actions/install-mdtablefix@abc\n",
        "      - run: make check-fmt\n",
    );
    assert_eq!(
        uninstalled_check_fmt(&one(text)),
        vec!["ci.yml:build-test".to_owned()]
    );
}

#[test]
fn a_chained_check_fmt_is_still_seen() {
    let text = concat!(
        "jobs:\n  build-test:\n    steps:\n",
        "      - run: make check-fmt && make test\n",
    );
    assert_eq!(
        uninstalled_check_fmt(&one(text)),
        vec!["ci.yml:build-test".to_owned()]
    );
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
    assert_eq!(uninstalled_check_fmt(&one(text)), Vec::<String>::new());
}

#[test]
fn narrowed_lint_globs_are_refused() {
    let text = concat!(
        "jobs:\n  lint:\n    steps:\n",
        "      - uses: DavidAnson/markdownlint-cli2-action@abc\n",
        "        with:\n          globs: 'docs/**/*.md'\n",
    );
    assert_eq!(lint_action_globs(&one(text)), (1, 1));
}

#[test]
fn a_globs_key_outside_with_does_not_set_the_input() {
    let text = concat!(
        "jobs:\n  lint:\n    steps:\n",
        "      - uses: DavidAnson/markdownlint-cli2-action@abc\n",
        "        env:\n          globs: '**/*.md'\n",
        "        with:\n          globs: 'docs/**/*.md'\n",
    );
    assert_eq!(lint_action_globs(&one(text)), (1, 1));
}

#[test]
fn a_lint_step_with_no_globs_input_is_refused() {
    let text = concat!(
        "jobs:\n  lint:\n    steps:\n",
        "      - uses: DavidAnson/markdownlint-cli2-action@abc\n",
        "        env:\n          globs: '**/*.md'\n",
    );
    assert_eq!(lint_action_globs(&one(text)), (1, 1));
}

#[test]
fn full_lint_globs_are_accepted() {
    let text = concat!(
        "jobs:\n  lint:\n    steps:\n",
        "      - uses: DavidAnson/markdownlint-cli2-action@abc\n",
        "        with:\n          globs: '**/*.md'\n",
    );
    assert_eq!(lint_action_globs(&one(text)), (1, 0));
}

#[rstest::rstest]
#[case::old("0.6.0", 1)]
#[case::unpinned("", 1)]
#[case::exact("0.6.1", 0)]
#[case::newer("0.7.0", 0)]
fn the_installer_must_pin_the_required_version(#[case] pinned: &str, #[case] weak: usize) {
    let version_line = if pinned.is_empty() {
        String::new()
    } else {
        format!("        with:\n          version: '{pinned}'\n")
    };
    let text = format!(
        "jobs:\n  build-test:\n    steps:\n      - uses: \
         leynos/shared-actions/.github/actions/install-mdtablefix@abc\n{version_line}"
    );
    assert_eq!(under_pinned_installers(&one(&text)).len(), weak);
}

/// The estate's canonical markdownlint rule settings, which every repository keeps.
fn canonical_config() -> Value {
    json!({
        "MD004": { "style": "dash" },
        "MD010": { "code_blocks": false },
        "MD013": {
            "line_length": 80,
            "code_block_line_length": 120,
            "tables": false,
            "headings": false
        },
        "MD029": { "style": "ordered" }
    })
}

/// Returns whether the configuration text keeps every canonical rule setting.
fn keeps_canonical_config(text: &str) -> bool {
    let Ok(parsed) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    let canonical = canonical_config();
    canonical.as_object().is_some_and(|rules| {
        rules
            .iter()
            .all(|(rule, settings)| parsed.pointer(&format!("/config/{rule}")) == Some(settings))
    })
}

#[test]
fn the_repository_keeps_the_canonical_markdownlint_config() {
    let text = manifest_dir()
        .and_then(|dir| dir.read_to_string(".markdownlint-cli2.jsonc"))
        .expect("the configuration is readable");
    assert!(keeps_canonical_config(&text));
}

#[rstest::rstest]
#[case::kept(r#"{"config":{"MD004":{"style":"dash"},"MD010":{"code_blocks":false},"MD013":{"line_length":80,"code_block_line_length":120,"tables":false,"headings":false},"MD029":{"style":"ordered"}}}"#, true)]
#[case::extra_rule_is_allowed(r#"{"config":{"MD004":{"style":"dash"},"MD010":{"code_blocks":false},"MD013":{"line_length":80,"code_block_line_length":120,"tables":false,"headings":false},"MD029":{"style":"ordered"},"MD033":false}}"#, true)]
#[case::wider_lines(r#"{"config":{"MD004":{"style":"dash"},"MD010":{"code_blocks":false},"MD013":{"line_length":120,"code_block_line_length":120,"tables":false,"headings":false},"MD029":{"style":"ordered"}}}"#, false)]
#[case::rule_dropped(r#"{"config":{"MD004":{"style":"dash"},"MD010":{"code_blocks":false},"MD029":{"style":"ordered"}}}"#, false)]
#[case::no_config(r#"{"ignores":[]}"#, false)]
#[case::not_json("not json", false)]
fn a_weakened_markdownlint_config_is_refused(#[case] text: &str, #[case] kept: bool) {
    assert_eq!(keeps_canonical_config(text), kept);
}
