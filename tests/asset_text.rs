#[path = "common/passage.rs"]
mod passage;

use factory::pack;
use passage::assert_unique_passage;

fn line_starting<'a>(label: &str, text: &'a str, start: &str) -> &'a str {
    let mut lines = text.lines().filter(|l| l.trim_start().starts_with(start));
    let line = lines
        .next()
        .unwrap_or_else(|| panic!("{label}: no line starts with {start:?}"));
    assert!(
        lines.next().is_none(),
        "{label}: two lines start with {start:?}"
    );
    line
}

#[test]
fn autonomy_names_research_notes_runbooks_and_sign_offs() {
    let text = pack::builder_autonomy();
    let start = text
        .find("## What an unattended run may write")
        .expect("the may-write section survives the builder cut");
    let end = start + text[start..].find("\n## Never, to get green").unwrap();
    let section = &text[start..end];
    for needle in [
        "repair anything the run itself wrote on this branch",
        "update project docs the run's FRs make factually wrong, such as research notes, runbooks and operations procedures, text only",
        "fix a defect a review found in a file the PRD ordered, later, with an `AD` marked `decide` as the creation had",
        "close or tick a human sign-off item, such as a checklist row, a signature or an approval, in any doc: that change is a proposal in the PR body",
        "**Never wins over every May.**",
        "a CI workflow the PRD ordered may get a defect fix a review found, never an edit that makes a red check pass",
        "`CLAUDE.md` and `docs/decisions.md` stay Never even when the run's FRs make them wrong",
        "for a builder \"anything the run itself wrote\" means its slice's files",
        "a checkpoint, fix or CI-fix worker touches only what its row or failure names",
    ] {
        assert_unique_passage("autonomy, what a run may write", section, needle);
    }
    let never = line_starting("autonomy", section, "**Never:**");
    assert_unique_passage("the Never list", never, "human sign-off item");
}

#[test]
fn a_behavioural_test_is_seen_red_once() {
    for needle in [
        "break the feature once (a revert or a stub) by hand, never through git: your work is uncommitted, so a `git checkout`, `restore` or `stash` loses it",
        "Run that test and see it go red, then restore the feature",
        "for each test your tasks' Acceptance names, and each `run` row",
        "Progress gets one line per test, naming how it failed",
        "These deliberate reds count in neither the `INNER` return line nor the Progress inner-loop runs and red",
        "A row only the harness drives just names how it fails with the feature removed",
    ] {
        assert_unique_passage("the builder contract", pack::CONTRACT, needle);
    }
}

#[test]
fn checkpoint_takes_its_expected_result_from_the_spec() {
    let prove = line_starting(
        "the checkpoint contract",
        pack::CONTRACT_CHECKPOINT,
        "2. **Prove**",
    );
    assert_unique_passage(
        "checkpoint Prove step",
        prove,
        "The expected result comes from the PRD, the spec or the input file, never from what the command printed.",
    );
}

#[test]
fn checkpoint_repairs_the_drive_a_covered_slice_broke() {
    let touch = line_starting(
        "the checkpoint contract",
        pack::CONTRACT_CHECKPOINT,
        "6. Touch nothing",
    );
    for needle in [
        "when a covered slice moved the surface a `.claude/skills/verify-*` recipe drives, repair that recipe, then drive the row and name the repair under **Fixed**",
        "Never drop a step an earlier checkpoint drove.",
        "The driver commits the repair with the checkpoint.",
        "A repair changes how the recipe drives the app, never what it expects",
    ] {
        assert_unique_passage("checkpoint touch rule", touch, needle);
    }
    let record = line_starting(
        "the checkpoint contract",
        pack::CONTRACT_CHECKPOINT,
        "4. **Record**",
    );
    assert_unique_passage(
        "checkpoint Record step",
        record,
        "`**Fixed:** {what, or none}`, which names any `verify-*` repair",
    );
}

#[test]
fn pr_body_names_full_gate_runs_and_inner_loops() {
    let gate = line_starting("the PR body", pack::PR_BODY, "- Gate `{command}`");
    for needle in [
        "summed from every checkpoint entry's `**Gate:**` line in the feature's log plus polish's own gate runs",
        "inner loops: {n} runs, {red} red, summed from the slices' Progress inner-loop lines",
    ] {
        assert_unique_passage("the PR body's Gate line", gate, needle);
    }
}
