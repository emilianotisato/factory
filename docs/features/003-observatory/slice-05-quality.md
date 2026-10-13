# Slice 05 — Quality signals (FR-24)

**Source**: [`prd.md`](prd.md) → §6 FR-24, two decisions ("A `FINDINGS` line in the polish return", "`report` gains a quality block"); §8 OPEN-1 and OPEN-5. **Depends on**: — (independent of 01-04: its own `runs` columns, its own event, its own report block; it appends tuples to the shared migration list and keys to the end of `EXTRA_KEYS`).
**Created**: 2026-10-13 · **Status**: pending

**Progress**:
- **Built (tasks 1-4).** `src/stream.rs` `Findings::from_result` (colon or not, keys in any order, all five or `None`) and `text()`, with a unit test. `src/db.rs`: eight guarded `runs` columns (`findings_*`, `decisions`, `decide`, `blocks`), `set_run_findings`, `set_run_log_counts`, and `RunRow.findings`/`decisions`/`decide`/`blocks`. `src/events.rs` `PolishFindingsMissing`. `src/closeout.rs` `polish()`: on a return whose `STATUS` parsed, stores the findings or nulls plus the event; `ship()` re-reads `docs status` and stores the log counts before `report::compute` (AD-501). `src/cli/report.rs`: `fix_commits`, `ci_red_rounds`, `ci_reruns`, `findings`, `decisions`, `decide`, `blocks` appended to `EXTRA_KEYS` and the JSON (`findings` an object or `null`), and the `quality:` line before the threshold lines (AD-502).
- **Contract, skill, stub (task 2).** `assets/contract-polish.md` carries `FINDINGS high=<n> medium=<n> low=<n> fixed=<n> parked=<n>` after `FIXED`, with one sentence on what it counts. `tests/stubs/claude` emits it by default and drops it on `skip: ["findings"]`. **Out-of-repo edit:** `~/Dropbox/dev/claude-code-skills/polish/SKILL.md` (§ Report, the unattended paragraph) was edited in place to end the return with the same line; the owner commits it in that repo.
- **Tests.** `tests/checkpoints.rs` red-red-green: `report --json` has `fix_commits == 2`. `tests/closeout.rs` red:test-green: `ci_red_rounds 1`, `ci_reruns 0`, `ci_cycles 1`. `tests/metrics.rs`: the default polish prints `findings: high=1 medium=2 low=3 fixed=4 parked=2` and the `quality:` line; `decisions`, `decide`, `blocks` equal `docs status 001 --json` `log` after the shipped run; new `a_polish_return_without_findings_is_nulls_and_one_event` (null row, one `polish_findings_missing`, `findings: —`, JSON `null`). The report unit test gains the seven keys and a `quality_line` test.
- **Inner loop: 7 runs, 5 red, about 13 min.** Every red was a sibling lane mid-edit in the shared tree: slice 02's per-worker text, slice 03's `pack_parts` compile error, slice 04's hanging `tests/knobs.rs` (a 10-minute run killed), and the text key-order check. The last green run: all 18 targets except `tests/knobs.rs`, 64 s. Still red at hand-back, and not this slice's: `report_prints_the_retro_keys_then_the_run_keys` until slices 03/04 add `knobs`, `hypothesis`, `pack_by_role` to the `EXTRA_KEYS` copy in `tests/metrics.rs` (Trap in `log-slice-05.md`).
- **Deviation caught by self-review.** A first fix made that key-order check tolerant of the sibling keys; that weakens a test this run did not write, so it is back to exact.
- **Prove (task 6)** left for CP-3: the FR-24 row.
- **Prove (task 6), driven at CP-1 (2026-10-12)**: the FR-24 row, green. Red, red, green gives `fix_commits: 2`. CI `red:test, green` gives `ci_red_rounds: 1` and `ci_reruns: 0`. A polish with `FINDINGS` gives `findings: high=1 medium=2 low=3 fixed=4 parked=2`. Without the line, the five columns are NULL, the report prints `findings: —`, and there is one `polish_findings_missing` event. Evidence: `evidence/cp-1-fr24-quality.txt` (four report outputs and the event line). This slice's code was left uncommitted under Untracked leftovers; it is in the CP-1 commit (AD-901).

- **Code-review fixes (2026-10-13).**
  - `Findings::from_result` read only the first line starting `FINDINGS`, and split on spaces only, so `FINDINGS below:` or `high=1, medium=2` stored nulls. It now takes the first line that parses, splitting on spaces or commas.

**Slice**: After this slice every run row carries the counts that say whether the builders left defects: what polish found by severity and what it did about it, how many fix commits the checkpoints needed, how many CI rounds went red, and the decisions and blocks the log holds. All at zero worker tokens beyond one return line. It leaves the retro's quality block to slice 06 and grades nothing with a model (prd §12).

## Current code / environment

- **Only `STATUS` is parsed from a return**: `src/stream.rs:126-141` `StatusHint::from_result` finds the first line with `strip_prefix("STATUS:")` and takes its first alphabetic word. The polish return block is `STATUS`, `FIXED`, `RETRO`, `PR_BODY`, `DECISIONS`, `BLOCK` (`assets/contract-polish.md:20-26`). The polish worker is spawned at `src/closeout.rs:261-267`, waited on and settled in `polish()` (`:185-339`), `PolishDone` emitted at `:336-339`; `settle()` (`:2095-2104`) is where its `Summary` reaches `finish_worker` and `record_exit`. The real polish return on run 024 ended `… | RETRO: written | PR_BODY: written | DECISIONS: AD-9901 | BLOCK: none` (verified 2026-10-13).
- **The skill that writes the return** lives outside this repo: `~/.claude/skills/polish/SKILL.md` (a symlink into `~/Dropbox/dev/claude-code-skills/polish`); its unattended return is described around `:166` and `:199`. The builder edits it but cannot commit it here (prd §2: "the line is added there now"; PRD-002 ports the skill later).
- **The stub polish return**: `tests/stubs/claude:350-360` `result([...])` with `STATUS`, `FIXED`, `RETRO`, `PR_BODY`, `DECISIONS`, `BLOCK`; `step.skip` (`["retro"]`, `["body"]`) already models a worker that skipped a line.
- **Fix, CI and gate records**: `FixCommitted { n, attempt, sha, files }` emitted at `src/checkpoint.rs:763-768`; `CiRed` at `src/closeout.rs:1160-1164`, `CiRerun` at `:1278-1282`, `CiFixCommitted` at `:1718`; `report::compute` counts with `count = |kind| events.iter().filter(|e| e.kind == kind).count()` and `ci_cycles: count("ci_fix_committed")` (`src/cli/report.rs:424`); `gate_*` keys count driver rows only (`:402`), `worker_gate_runs`/`worker_gate_red` are `EXTRA_KEYS` (`:81`, `:464`); `score: None` always (`:420`).
- **Decisions are counted but never read by the driver**: `docs status --json` → `Status.log: LogSummary { decisions: u32, flagged: Vec<u32>, decide: Vec<u32>, blocks: u32 }` (`src/docs.rs:270-276`); `tests/docs.rs:34` asserts `s.log.decisions == 0` on its fixture. The ship step (`src/closeout.rs:526+`) has `board: &Status` in hand and calls `report::compute` at `:574` before `fill_retro` (`:590`).
- **Test scenarios the acceptance names**: `tests/checkpoints.rs:599-640` `red_red_green_runs_two_fix_workers_and_three_gates_before_the_cp` (`script_gate(["red","red","green"])`, two fix commits); `tests/closeout.rs:726` `a_red_check_gets_one_fix_round_then_green` (`script_ci(["red:test","green"])`, one CI fix). Both assert through the store or events already; the new assertions go through `factory report <id> --json`.
- **Schema and report conventions**: append tuples to the guarded migration list (`src/db.rs:257-267`), set `SCHEMA_VERSION` to 2 if still 1; append keys at the end of `EXTRA_KEYS` (`src/cli/report.rs:80-89`), the unit test asserts order and `lines()[16]`, `[17]`, `[21]`.
- **Deliberate non-changes** — *(prd §6: "counts only, no composite score"; rejected "parsing the `AD` entries" and "fix commits only". §12: "Not handled: a `factory grade` review worker"; "Not handled: owner review signals"; "Not handled: post-merge churn and production bugs". `score` stays the owner's and stays empty. Accepted cost: "these say the builders left defects, not where".)*

## Tasks

### 1. Parse the `FINDINGS` line and store it — high — blocked by: —
**Source**: §6 FR-24: "A `FINDINGS` line in the polish return: `FINDINGS high=<n> medium=<n> low=<n> fixed=<n> parked=<n>`, parsed like `STATUS`, stored as `runs.findings_high`, `findings_medium`, `findings_low`, `findings_fixed`, `findings_parked`. … A missing line gives nulls and a `PolishFindingsMissing` event."

`src/stream.rs`: `Findings { high, medium, low, fixed, parked: u32 }` with `from_result(text) -> Option<Findings>`, beside `StatusHint::from_result`: the first line whose trimmed start is `FINDINGS` (with or without a colon), then `key=<digits>` pairs in any order; all five keys required, else `None`. `src/db.rs`: five `("runs", "findings_<k>", "INTEGER")` tuples, `set_run_findings(run_id, Option<Findings>)`, the five on `RunRow`. `src/events.rs`: `PolishFindingsMissing { session_id: String }`. `src/closeout.rs` `polish()`: when the polish worker returned a result (its `STATUS` parsed), store the findings; on `None` store nulls and emit the event once. A dead or blocked polish stores nothing and emits nothing: there was no return to parse.

**Acceptance:** `stream.rs` unit tests: `"STATUS: done\nFINDINGS high=1 medium=2 low=3 fixed=4 parked=2\nBLOCK: none"` → the five values; a line missing `parked` → `None`; `FINDINGS: high=0 …` with a colon → parsed. Harness (task 4): the stub's default polish fills the five columns; `skip: ["findings"]` leaves nulls and one `polish_findings_missing` line in `events.jsonl`.

### 2. The contract, the skill and the stub carry the line — high — blocked by: —
**Source**: §6 FR-24: "Added to `assets/contract-polish.md` and to the `/polish` skill's return."; §2: "The polish contract and the `/polish` skill: one new return line (FR-24). A polish worker that omits it produces nulls, never a failed run."

`assets/contract-polish.md:20-26`: add `FINDINGS high=<n> medium=<n> low=<n> fixed=<n> parked=<n>` after `FIXED`, with one sentence: counts after dedupe and triage, by severity as found, then how many were fixed in this pass and how many parked. `~/.claude/skills/polish/SKILL.md`, the unattended return (about `:199`): the same line in the return block. `tests/stubs/claude:350-360`: `FINDINGS high=1 medium=2 low=3 fixed=4 parked=2` after `FIXED`, omitted when `step.skip` contains `findings`.

The trap: the skill file is outside this repo and the builder's commit cannot include it. Edit it, and write in Progress that it was edited in place under `~/Dropbox/dev/claude-code-skills/polish/SKILL.md`, so the owner commits it there.

**Acceptance:** `assets/contract-polish.md` names the line and its five keys; `tests/stubs/claude` emits it by default and omits it on `skip: ["findings"]` (asserted through task 4's scenarios); Progress records the out-of-repo edit.

### 3. Count fixes, CI rounds and decisions on the run row — high — blocked by: —
**Source**: §6 FR-24: "`report` gains a quality block, counts only, no composite score: `gate_red` (driver, exists), `worker_gate_red` (… moves into the block), `fix_commits` (`FixCommitted` events), `ci_red_rounds` (`CiRed` events), `ci_reruns` (`CiRerun` events), the five `findings_*`, and `decisions`, `decide`, `blocks` read from `LogSummary` at ship and stored on the run row."

`src/db.rs`: `("runs", "decisions", "INTEGER")`, `("runs", "decide", "INTEGER")`, `("runs", "blocks", "INTEGER")`, `set_run_log_counts`. `src/closeout.rs` `ship()`: before `report::compute` at `:574`, store `board.log.decisions`, `board.log.decide.len()`, `board.log.blocks` from the `Status` the ship already holds (re-read `docs.status(feature)` there if the held one predates polish, since polish writes `AD` entries). `src/cli/report.rs` `compute()`: `fix_commits: count("fix_committed")`, `ci_red_rounds: count("ci_red")`, `ci_reruns: count("ci_rerun")`, the five findings and the three log counts from the `runs` row, all `Option` where the row may be null.

**Acceptance:** `tests/checkpoints.rs` red-red-green scenario: `factory report <id> --json` has `fix_commits == 2`; `tests/closeout.rs` red:test-green scenario: `ci_red_rounds == 1`, `ci_reruns == 0`, `ci_cycles == 1`; after a shipped fixture run, `decisions` equals the `log.decisions` that `docs status 001 --json` prints for that fixture at that moment.

### 4. `report` prints the quality block — high — blocked by: 1, 3
**Source**: §6 FR-24 Acceptance: "the stub polish recording with a `FINDINGS` line fills the five columns and `report` prints them; a recording without the line leaves nulls, emits the event and prints `findings: —`".

`src/cli/report.rs`. `EXTRA_KEYS` gains, appended last: `fix_commits`, `ci_red_rounds`, `ci_reruns`, `findings`, `decisions`, `decide`, `blocks`. The text report prints one `quality:` line: `gate_red 0 · worker_gate_red 0 · fix_commits 2 · ci_red_rounds 1 · ci_reruns 0 · findings high=1 medium=2 low=3 fixed=4 parked=2 · decisions 3 decide 0 blocks 0`, with `findings: —` when the five are null and `—` for any null count; `worker_gate_red` keeps its own key in JSON and is repeated inside the line (it "moves into the block" in the text, nothing is removed from `--json`). JSON carries `findings` as an object or `null`.

**Acceptance:** the report unit test's order assertion gains the seven keys last with `lines()[16]`, `[17]`, `[21]` unchanged; `tests/metrics.rs`: the default polish prints `findings high=1 medium=2 low=3 fixed=4 parked=2`, the `skip: ["findings"]` run prints `findings: —` and `--json` has `"findings": null`.

### 5. Verify — medium — blocked by: 1-4
`cargo test` (the inner loop; the full gate `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` runs at this slice's checkpoint).

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
Rows from [`prd.md`](prd.md) §9 this slice must prove, below the UI: the **FR-24 row**: `run` the "red, red, green" gate scenario, the "red:test, green" CI scenario and a polish with `FINDINGS` → the quality block prints `fix_commits = 2`, `ci_red_rounds = 1`, `ci_reruns = 0` and the five findings; polish without the line → nulls and the event. Driven at CP-3 together with the full gate; the test transcript goes in this slice's **Progress**.

**Acceptance:** the FR-24 row has its test transcript attached in Progress, with the three `report` outputs and the `polish_findings_missing` event line.
