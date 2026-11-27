---
number: 029
slug: review-and-polish-speedup
title: "Sharper, cheaper review collect and polish"
type: feature
created: 2026-11-26
author: Emiliano Tisato
authorized_tests: ["tests/review.rs::collect_owes_the_fallback_when_every_pass_timed_out", "tests/review.rs::no_feature_keeps_the_prompt_as_today"]
summary: |
  Makes `factory review collect`, `factory review second`, and `/polish` faster and sharper: drops `docs/features/**` planning docs from code review passes while keeping docs-only diffs reviewed; asks each diff pass for a structured `file:line | severity | claim` summary table and merges all passes mechanically into a `### Claims index` at the top of `## Second opinion` (kept out of `# Collected` by `pack::collected`); classifies unreviewed pass failures (`usage limit`, `auth`, `no such model`, `timeout`, `stopped`) on the Plan line; records `reviewed_head` in `state.toml` and intersects `--since` with `<base>...HEAD` (falling back cleanly if rewritten); includes answered asks in the settled block, adds `factory review settled <NNN>`, and unifies `SIGINT`/`SIGTERM`/`SIGHUP` forwarding across `review` and `checkpoint`.
---

# PRD-029 — Sharper, cheaper review collect and polish

> **Interviewed 2026-11-26** (absorbing `2026-11-23-second-opinion-don-t-review-a-feature-s-own-docs-a.md`, `2026-11-26-review-collect-emit-structured-one-line-per-claim.md`, `2026-11-23-review-collect-say-why-a-pass-came-back-unreviewed.md`, `2026-11-12-polish-since-start-before-the-last-polish-s-own-fi.md`, `2026-11-12-settled-block-include-answered-asks-beside-decisio.md`, `2026-11-23-a-verb-that-prints-a-feature-s-settled-block.md`, and `2026-11-26-unify-sigint-sigterm-handlers-between-review-colle.md`). Every `file:line` below was verified against `main` at `9bbf37e`. Revised 2026-11-26 after cross-vendor adversarial pass (`claude:opus`, log `20261126-235528`).

## 1. Why now

Four real time and token sinks showed up repeatedly across PRD-014 through PRD-028 retros:

- **Planning docs burn whole code-review passes.** `review::skip` (`src/review.rs:299-332`) drops `log.jsonl` and `state.toml` (`tool_written`), but still sends `prd.md`, `slice-NN-*.md`, `retro.md`, and `qa-guide.md` as code diffs. On PRD-014, 150 of 252 lines were planning docs; on PRD-015, pass 1 was 579 lines of planning docs; on PRD-024, passes 1–2 spent 28 minutes re-reviewing code through its markdown docs.
- **Multi-pass prose walls make `/polish` dedupe the slowest step.** Across PRD-020, PRD-022, and PRD-027 retros, 4–10 second-opinion passes returned long prose paragraphs with overlapping claims and no top-level summary index.
- **Unreviewed passes hide *why* they failed, and `polish --since` misses round-1 polish fixes while pulling in merged `main`.** On PRD-015/017/018, Cursor's monthly quota ran out while `--list-models` still passed; `collect.md` (`src/collect.rs:314-317`) only printed `no pass of N counted`. Meanwhile, `settled::last_reviewed` (`src/settled.rs:30-46`) anchors `--since` at `R` (the commit that changed `reviewed` in `state.toml`), which leaves out round-1 polish's own fixes, drags in unrelated `main` files after a merge via raw `R..HEAD` tree diff, and fails if a rebase rewrites `R`.
- **Answered asks vanish from settled blocks, fallback briefs have no CLI verb to print the settled block, and `review`/`checkpoint` signal handlers clobber each other.** `settled::decisions` (`src/settled.rs:49-93`) skips `Kind::Ask` entries even after `Kind::Answer` settles them; fallback polish passes had to dig the settled block out of `.jsonl` logs because no CLI verb prints `settled::for_feature`; and `src/review.rs:511-529` and `src/checkpoint.rs:51-73` install separate `Once`-guarded `libc::sigaction` handlers that overwrite each other in the same driver process.

**What this leaves alone.** No extra LLM post-pass summarizer (claims indexing is 100% deterministic Rust parsing of pass tables), no changing `leaks` scan scope (`leaks` still scans `<base>...HEAD`), and no removing the four passages in `skills/polish/SKILL.md` pinned by `tests/skills_bundle.rs:428-433`.

## 2. Blast radius

- **`tests/review.rs::collect_owes_the_fallback_when_every_pass_timed_out` (authorized).** Line 704 currently asserts `"\nsecond opinion: fallback owed — unreviewed: no pass of 2 counted\n"`. FR-3a puts the classified failure cause on that line (`fallback owed — unreviewed: timeout (no pass of 2 counted)`), so `:704` updates to match.
- **`tests/review.rs::no_feature_keeps_the_prompt_as_today` (authorized — caught 2026-11-26 in adversarial pass).** Lines 1016-1046 compare the full `diff_prompt` string with `assert_eq!`; FR-2 appends the `### Summary` table instruction to `diff_prompt` (`src/review.rs:1446-1451`), so `:1040-1043` updates to include the new sentence.
- **`pack::collected` unanchored local-review invariant (`src/pack.rs:126-165` — caught 2026-11-26 in adversarial pass).** `pack::collected` currently copies every non-empty line under `## Second opinion` before the first `### Pass ` into `# Collected`. Putting `### Claims index` before `### Pass 1` would leak every claim row into `# Collected` before the local review runs unless `pack::collected` stops copying at ANY `### ` heading (not just `### Pass `). FR-2 updates `pack::collected` to stop copying at any `### ` heading while still collecting `- Pass ...` status lines.
- **`review::skip` backwards compatibility.** `tests/review.rs:728` and `:787` test `LOG` (`log.jsonl`), `STATE` (`state.toml`), `testdata/a.xsd`, and `src/a.rs`. Keeping `tool_written` checked first (`"tool-written"`) and skipping other `docs/features/**` files (`"feature-doc"`) ONLY when at least one non-feature-doc file remains kept leaves both existing tests green untouched. Update `Skip.skipped` doc comment (`src/review.rs:269`) to include `feature-doc`.
- **`state.toml` schema forward-compatibility (`src/docs/state.rs:53-55` — clarified 2026-11-26 after adversarial pass).** `Raw` uses `#[serde(deny_unknown_fields)]`: existing `state.toml` files without `reviewed_head` continue to parse cleanly on the new binary, and `settled::last_reviewed` falls back to `git log -1 -G '^reviewed ='` when `reviewed_head` is absent (leaving `tests/review.rs:1150` and `:1165` green untouched). However, an **older** `factory` binary (< PRD-029) will fail to parse a `state.toml` that carries `reviewed_head`, so upgrade/install the new binary before sharing branches written by PRD-029. Clearing `reviewed=""` also clears `reviewed_head`.
- **`skills/polish/SKILL.md` & `skills/bundle.lock`.** Updating `skills/polish/SKILL.md` to prefer `reviewed_head` for attended `--since` (while keeping the `git log -1 --format=%H -G '^reviewed' -- docs/features/<NNN>-*/state.toml` fallback pinned by `tests/skills_bundle.rs:431`), reference `factory review settled <NNN>` for fallback briefs, and read `### Claims index` in §2 requires regenerating `skills/bundle.lock` via `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` (`v0.18.0` is untagged, so no version bump is required).

## 3. FR-1 — Skip `docs/features/**` planning docs in code-review passes

**Today.** `review::skip` (`src/review.rs:299-332`) drops only `docs/features/<dir>/log.jsonl` and `docs/features/<dir>/state.toml` (`tool_written`) plus `.gitattributes` `linguist-vendored`/`linguist-generated` paths. Planning docs (`prd.md`, `slice-NN-*.md`, `retro.md`, `qa-guide.md`) stay in `kept`, inflating pass counts and review time whenever code also changed.

**Decided 2026-11-26.**

- Add `feature_doc(path: &str) -> bool` in `src/review.rs`: matches any path under `docs/features/` (`path.starts_with("docs/features/")`) after `tool_written` has already claimed `log.jsonl` and `state.toml`.
- In `review::skip(root: &Path, files: Vec<(String, u64)>) -> Skip`:
  - First classify `tool_written` (`"tool-written"`) and `.gitattributes` `linguist-vendored`/`linguist-generated` (`"vendored"` / `"generated"`) as today.
  - Inspect the remaining candidate files: if **at least one** candidate file is NOT a `feature_doc`, move every `feature_doc` candidate into `out.skipped` with reason `"feature-doc"`, keeping only non-feature-doc files in `out.kept`.
  - If **every** remaining candidate file is a `feature_doc` (a docs-only PR/branch), keep them all in `out.kept` so docs-only diffs are still reviewed as before.
  - Update the `Skip.skipped` doc comment (`src/review.rs:269`) to list `tool-written`, `feature-doc`, `vendored`, or `generated`.
- Applies automatically to both `factory review collect` (`src/collect.rs:134`) and `factory review second --diff` (`src/cli/review.rs:153`) since both call `review::skip`.

**Accepted cost.** When a feature branch edits both code and `docs/features/**` (e.g., PRD/slices/retro), the second-opinion reviewer does not read those markdown diffs as code passes (the PRD's Decisions log and Non-goals already arrive in the settled block). A docs-only branch continues to be reviewed in full.

**Acceptance.** Integration test `review_skip_drops_feature_docs_when_code_changed_but_keeps_docs_only_diffs` proves: (1) a branch changing `src/a.rs` + `docs/features/001-x/prd.md` + `docs/features/001-x/slice-01-a.md` + `docs/features/001-x/log.jsonl` lists `prd.md` and `slice-01-a.md` as `(feature-doc)` under `skipped:`, `log.jsonl` as `(tool-written)`, and sends only `src/a.rs` to the reviewer pass diff; (2) a branch changing ONLY `docs/features/001-x/prd.md` + `docs/features/001-x/slice-01-a.md` (with `log.jsonl`) skips `log.jsonl` (`tool-written`) and still reviews `prd.md` and `slice-01-a.md`.

## 4. FR-2 — Structured one-line-per-claim severity index in `collect.md`

**Today.** `review::diff_prompt` (`src/review.rs:1446-1451`) asks for defects at `file:line` with a failure scenario, with no structured table format; `collect::second_opinion` (`src/collect.rs:305-321`) concatenates raw `### Pass` prose sections with no merged summary table; and `pack::collected` (`src/pack.rs:150-159`) copies every non-empty line before `### Pass ` into `# Collected`.

**Decided 2026-11-26.**

- **Prompt format (`src/review.rs:1446-1451`):** extend `diff_prompt` with one short instruction asking the reviewer to end its response with a `### Summary` markdown table with columns `| file:line | severity | claim |` (severity one of `high`, `medium`, `low`; omit the table if there are no defects). Keep the existing `"Adversarial code review. {what} {against} is in {}: read it, ..."` prefix intact and update authorized test `tests/review.rs::no_feature_keeps_the_prompt_as_today`.
- **Deterministic parser & merged index (`src/collect.rs`):**
  - Add a pure helper `pub fn extract_claims(pass_n: usize, text: &str) -> Vec<Claim>` (`Claim { pass: usize, location: String, severity: Severity, summary: String }`, where `Severity` orders `High > Medium > Low` case-insensitively):
    - Locate a markdown table (either under a `### Summary` / `## Summary` heading or any pipe-delimited table whose header row has 3 columns matching location/file, severity, and claim/defect/summary).
    - Parse each data row (skipping the `|---|---|---|` separator row and `"none"`/`"—"` placeholder rows), matching severity case-insensitively (`high`, `medium`, `low`; ignoring rows whose severity column is not one of those three).
    - Sanitize newlines and literal `|` inside cells so a malformed cell cannot break the merged markdown table.
  - In `collect::second_opinion` (`src/collect.rs:305-321`): collect claims from every counted pass, sort by `(severity desc, location asc, pass asc)`, and when at least one claim is extracted, render a `### Claims index` markdown table (`| severity | file:line | pass | claim |`) right below `exit: 0` / `log: ...` and above the first `### Pass` section. When no pass emitted parseable table rows, omit `### Claims index` cleanly.
- **Keep `# Collected` unanchored (`src/pack.rs:148-159`):** update `pack::collected` so once ANY `### ` heading appears under `## Second opinion` (including `### Claims index` or `### Pass `), non-pass lines are NOT copied into `# Collected` (while `### Pass ...` status lines continue to be collected as `- Pass ...`).
- Update `skills/polish/SKILL.md` §2 (`:89`) to tell `/polish` to start dedupe from `### Claims index` when present and verify each row against `### Pass` and the code.

**Accepted cost.** Reviewers may occasionally deviate from the requested table format; deterministic parsing handles standard markdown tables and degrades silently to the full `### Pass` prose with zero extra LLM calls or latency.

**Acceptance.** Integration & unit test `collect_merges_structured_claims_index_sorted_by_severity` (using a fresh temporary stub script created inside the test's tempdir — never modifying `tests/stubs/cursor-agent` — plus a `src/pack.rs` unit test) proves: (1) `diff_prompt` includes the `| file:line | severity | claim |` instruction; (2) when 2 counted passes return prose + `### Summary` tables containing mixed `low`, `high`, and `medium` rows, `collect.md` includes a `### Claims index` section before `### Pass 1` with all rows sorted `high` → `medium` → `low` and tagged with their pass number; (3) `pack::collected` strips `### Claims index` and its rows from `# Collected` while keeping `exit:`, `log:`, and `- Pass ...` lines; (4) when passes return prose with no table, `collect.md` omits `### Claims index` and still includes all `### Pass` prose cleanly.

## 5. FR-3 — Unreviewed failure-cause classification and fixed `polish --since`

**Today.**
- `collect::second_opinion` (`src/collect.rs:314-317`) writes `fallback owed — unreviewed: no pass of N counted` whenever `counted == 0`, discarding whether passes hit a usage limit, auth error, missing model, timeout, or stop. `review::probe_resolved` (`src/review.rs:671-739`) checks Cursor login/auth text, not usage-limit banners.
- `state::State` (`src/docs/state.rs:26-61`) stores only `reviewed: Option<String>` (a date). `settled::last_reviewed` (`src/settled.rs:30-46`) finds the commit `R` that changed `reviewed`, so round 2's `Range::Since(R)` (`R..HEAD`) misses round-1 polish's own fixes (which landed at/before `R`), includes every file `main` changed if `main` was merged after `R`, and errors out if a rebase removed `R`.

**Decided 2026-11-26.**

- **FR-3a — Unreviewed failure-cause classification (`src/review.rs`, `src/collect.rs`):**
  - Add `pub fn classify_failure(ran: &[Ran]) -> Option<&'static str>` in `src/review.rs` inspecting each failed `Ran` (`Ended::Stopped`, `Ended::Killed`, `o.parsed.error`, `o.stderr`, and `Err(e)`):
    - Classify each failed pass into one of five causes (case-insensitive):
      1. `"usage limit"` — matches `"usage limit"`, `"rate limit"`, `"quota"`, `"too many requests"`, `"429"`, `"out of credits"`, `"credit balance"`.
      2. `"auth"` — matches `"authentication required"`, `"not logged in"`, `"unauthorized"`, `"invalid api key"`, `"login"`, `"401"`, `"403"`.
      3. `"no such model"` — matches `"model not found"`, `"no such model"`, `"unknown model"`, `"invalid model"`, `"not in --list-models"`.
      4. `"timeout"` — when `Ended::Killed`.
      5. `"stopped"` — when `Ended::Stopped`.
    - When multiple failed passes exist, pick the single highest-priority cause present across the failed passes in strict deterministic priority order: `usage limit > auth > no such model > timeout > stopped`.
    - In `collect::second_opinion` (`src/collect.rs:314-317`), when `counted == 0`: if `classify_failure(&ran)` returns `Some(cause)`, format the reason as `"{cause} (no pass of {} counted)"` (e.g. `fallback owed — unreviewed: usage limit (no pass of 6 counted)`, `fallback owed — unreviewed: timeout (no pass of 2 counted)`); if `None`, keep `"no pass of {} counted"` (appending `: {first_err}` trimmed to 120 chars if a failed pass carried non-empty error/stderr text).
    - In `review::probe_resolved` (`src/review.rs:723-731`): on **non-zero** exit, prefix with `"usage limit: "` if the error text classifies as usage limit; on **exit 0** for Cursor ONLY, check explicit phrases `"hit your usage limit"` / `"usage limit reached"` (never bare `"429"` or `"quota"` on exit 0, so healthy `--list-models` or `--version` output can never false-positive!).
- **FR-3b — `reviewed_head` in `state.toml` and branch-intersected `--since` (`src/docs/state.rs`, `src/settled.rs`, `src/collect.rs`, `src/closeout.rs`, `skills/polish/SKILL.md`):**
  - Add `pub reviewed_head: Option<String>` to `state::State` and `Raw` (`src/docs/state.rs:26-61`), rendered as `reviewed_head = "<sha>"` right after `reviewed` when `Some`.
  - Accept `reviewed_head` in `state::apply` (`:175-179`): empty string clears to `None`; otherwise require 7..=40 ASCII hex characters (refuse anything else clearly, and include `reviewed_head` in `unknown(key)`). Clearing `reviewed=""` also clears `s.reviewed_head = None` so resetting `reviewed` always resets `--since` cleanly.
  - Add `pub fn head_of(text: &str) -> Option<&str>` in `src/collect.rs` extracting `<sha>` from line 1 `<!-- head: <sha> -->`.
  - In `closeout::ship` (`src/closeout.rs:1055-1168`, before `docs.ship(&feature)` and before the ship commit): if `run.dir().join(COLLECT_FILE)` exists, parse its `<!-- head: <sha> -->`, and when `state.reviewed` is set, write `reviewed_head=<sha>` via `docs.set(&feature, &[("reviewed_head", head)])` so the ship commit records the exact HEAD the collect reviewed (before round-1 polish's fixes).
  - In `settled::last_reviewed(checkout: &Path, feature: &str) -> Option<String>` (`src/settled.rs:30-46`): when `state.reviewed.is_some()`, return `state.reviewed_head` if present; otherwise keep the `git log -1 --format=%H -G '^reviewed ='` fallback (matching `reviewed =` strictly, not `reviewed_head =`) for older features.
  - In `collect::collect` (`src/collect.rs:123-135`): when `opts.since` is `Some(c)`:
    1. Check whether `c` resolves to a valid commit in `root` (`git rev-parse --verify --quiet <c>^{commit}`). If not (e.g. rewritten away by a rebase), fall back cleanly to `Range::Base(opts.base)` instead of failing.
    2. Compute branch paths from `review::numstat(root, Range::Base(opts.base), &[])?` and since files from `review::numstat(root, Range::Since(c), &[])?`. Filter the since files to only those whose path is present in the branch's `<base>...HEAD` numstat, so files modified only by merging `main` into the feature branch after `c` are excluded from the since passes.
  - In `skills/polish/SKILL.md:33`: tell attended `/polish` to pass `--since <reviewed_head>` when `state.toml` sets `reviewed_head`, falling back to `git log -1 --format=%H -G '^reviewed' -- docs/features/<NNN>-*/state.toml` when `reviewed_head` is absent (preserving the passage checked by `tests/skills_bundle.rs:431`).

**Accepted cost.** (1) Failure-cause classification uses pragmatic substring matching on provider error/stderr lines; novel vendor messages fall back cleanly to the first stderr/error line rather than hiding information. (2) When a branch merges `main` AND also edited the exact same file on the feature branch after `reviewed_head`, `c..HEAD` still shows the net change to that shared file (path-level intersection eliminates all unrelated `main` files without 3-way range-diff).

**Acceptance.** Integration test `collect_classifies_unreviewed_causes_and_since_ignores_merged_main_and_uses_reviewed_head` (plus updated `tests/review.rs::collect_owes_the_fallback_when_every_pass_timed_out`) proves: (1) when all passes exit 1 with `"You've hit your usage limit"` (or auth error / missing model), `collect.md`'s Plan line says `fallback owed — unreviewed: usage limit (no pass of N counted)` (and `timeout` for timeouts), while a healthy probe listing a model named `model-429-quota` still succeeds; (2) setting `reviewed_head=<sha>` in `state.toml` makes `settled::last_reviewed` return `<sha>` (including round-1 polish fixes committed after `<sha>`), and clearing `reviewed=` clears `reviewed_head`; (3) merging an unrelated commit on `main` after `reviewed_head` does NOT include `main`'s files in `collect --base main --since <sha>`, while a rebased-away `--since <dead-sha>` falls back cleanly to `main...HEAD` instead of erroring; (4) `skills/polish/SKILL.md` mentions `reviewed_head` in §1 while `tests/skills_bundle.rs` passes.

## 6. FR-4 — Answered asks in settled block, `factory review settled <NNN>`, and unified signal forwarding

**Today.**
- `settled::decisions` (`src/settled.rs:49-93`) filters strictly on `e.kind == log::Kind::Decision`, dropping `Kind::Ask` entries even when a `Kind::Answer` with `reference == Some(id)` settled them.
- `cli::review::ReviewVerb` (`src/cli/review.rs:17-23`) has only `Second` and `Collect` — no CLI verb prints `settled::for_feature`.
- `src/review.rs:471-529` (`CHILD_GROUPS` + `forward_signals`) and `src/checkpoint.rs:36-73` (`ACTIVE_GATE_PGID` + `forward_gate_signals`) each call `Once::call_once` to register their own `libc::sigaction` handler on `SIGINT`, `SIGTERM`, `SIGHUP`, so whichever runs second overwrites the first handler in the same process.

**Decided 2026-11-26.**

- **Include answered asks in `settled::decisions` (`src/settled.rs:49-93`):**
  - Include both `Kind::Decision` and `Kind::Ask` in entry order (skipping superseded ids as today).
  - For a `Kind::Ask` entry with `id`: find the latest `Kind::Answer` entry with `r.reference.as_deref() == Some(id.as_str())`. If unanswered, skip it; if answered, render `"- {id} — {}: answered — {}"` (using the trimmed answer body, or `"answered"` if body is empty).
- **Add `factory review settled <NNN>` (`src/cli/review.rs`):**
  - Add `Settled { feature: String }` variant to `ReviewVerb` (`src/cli/review.rs:17-23`).
  - Resolve `root = paths::repo_root(&cwd)?`, verify the feature exists via `retro::feature_dir(&root, &feature)` (returning exit `1` with a clear error if unknown), and write `settled::block(&dir)` to `out` (printing nothing if the feature exists with nothing settled yet) and exit `0`.
  - Update `skills/polish/SKILL.md:72-81` so fallback briefs tell `/polish` to run `factory review settled <NNN>` to fetch the settled block (preserving all passages checked by `tests/skills_bundle.rs:416-436`, and regenerating `skills/bundle.lock`).
- **Unify signal forwarding into `src/signals.rs` (`src/signals.rs`, `src/lib.rs`, `src/review.rs`, `src/checkpoint.rs`):**
  - Create a small shared `src/signals.rs` holding one `SLOTS = 256` `AtomicI32` table (`CHILD_GROUPS`), one `pub struct Guard(&'static AtomicI32)` (`pub fn claim() -> Result<Guard>`, `pub fn set(&self, pgid: i32)`, cleared to `0` on `Drop`), and one `Once`-guarded `pub fn install()` registering `SIGINT`, `SIGTERM`, `SIGHUP` (preserving `SIG_IGN` for `nohup`, killing every active `pgid > 0` with `libc::kill(-pgid, libc::SIGKILL)`, then resetting `SIG_DFL` and re-raising).
  - Replace the duplicate `CHILD_GROUPS`/`forward_signals` in `src/review.rs:470-529` and `ACTIVE_GATE_PGID`/`forward_gate_signals` in `src/checkpoint.rs:36-73` with `crate::signals::install()` + `crate::signals::Guard::claim()` (neither file retains any `libc::sigaction` call).

**Accepted cost.** A `Guard` table of 256 slots is shared across concurrent review passes and the active gate in one process (far above realistic concurrency of ~10–20). Unanswered asks remain excluded from the settled block because they are not settled yet.

**Acceptance.** Integration & unit test `settled_includes_answered_asks_prints_via_cli_and_signals_are_unified` proves: (1) `settled::block` includes an answered `Kind::Ask` as `- AD-n — <title>: answered — <answer>` and omits an unanswered `Kind::Ask`; (2) `factory review settled 001` prints the exact block that `settled::for_feature` returns and exits `0` (and exits `1` for an unknown feature); (3) `skills/polish/SKILL.md` mentions `factory review settled <NNN>` exactly once while `tests/skills_bundle.rs` passes; (4) the integration test re-executes itself as a helper subprocess (via an env var such as `FACTORY_SIGNALS_TEST_CHILD=1`) that spawns two child process groups registered under `signals::Guard` + `signals::install()`, receives `SIGTERM`, and the parent verifies BOTH child process groups are dead (`libc::kill(pid, 0) != 0`), AND verifies neither `src/review.rs` nor `src/checkpoint.rs` calls `libc::sigaction`.

## 7. Decisions log

No `OPEN-n`. Every fork was settled on 2026-11-26 and lives in its FR:

- Drop all `docs/features/**` files (as `feature-doc`) in `review::skip` whenever at least one non-feature-doc file remains kept; keep docs-only diffs reviewed (§3).
- Prompt each diff pass for a `| file:line | severity | claim |` summary table, merge all counted passes deterministically into `### Claims index` at the top of `## Second opinion`, and keep `pack::collected` unanchored by stopping before any `### ` heading (§4).
- Classify unreviewed pass/probe failures with deterministic priority (`usage limit > auth > no such model > timeout > stopped`) without false-positiving healthy exit-0 probes, record `reviewed_head` in `state.toml` at ship time (and prefer it in attended `/polish`), and intersect `--since` paths with `<base>...HEAD` with clean rebase fallback (§5).
- Include answered asks in `settled::decisions`, add `factory review settled <NNN>`, update `skills/polish/SKILL.md` (+ `skills/bundle.lock`), and unify `SIGINT`/`SIGTERM`/`SIGHUP` forwarding in `src/signals.rs` (§6).

## 8. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit` *(declared in `docs/project.toml`)*
**Surface**: `factory review collect`, `factory review second`, `factory review settled`, `factory docs set <NNN> reviewed_head=<sha>`, `pack::collected`, and signal forwarding (`src/signals.rs`). No GUI: every FR is proven below it.
**Harness**: `tests/review_speedup.rs` (new), `tests/review.rs`, `tests/skills_bundle.rs`, `src/pack.rs`, `src/settled.rs`, `src/collect.rs`, `src/review.rs`, `src/signals.rs`.
**Seed state**: fixture repos with mixed code + `docs/features/**` diffs, docs-only diffs, temporary stub scripts emitting `### Summary` tables or usage-limit/auth/model errors, branches with `reviewed_head` + merged `main` commits, and logs with answered/unanswered asks.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `review_skip_drops_feature_docs_when_code_changed_but_keeps_docs_only_diffs` | test transcript | no |
| FR-2 | run `collect_merges_structured_claims_index_sorted_by_severity` | test transcript | no |
| FR-3 | run `collect_classifies_unreviewed_causes_and_since_ignores_merged_main_and_uses_reviewed_head` | test transcript | no |
| FR-4 | run `settled_includes_answered_asks_prints_via_cli_and_signals_are_unified` | test transcript | no |

No row needs a human, so no `qa-guide.md` is generated.

## 9. Slices

Four vertical slices with zero overlapping write files between concurrent slices:

- **Slice 01 — Skip feature docs in code passes and unify signal forwarding (`FR-1` + `FR-4` signals, deps: —):** drops `docs/features/**` as `feature-doc` in `review::skip` when non-feature-doc files exist (preserving docs-only diffs) and unifies `SIGINT`/`SIGTERM`/`SIGHUP` child-group forwarding across `src/review.rs` and `src/checkpoint.rs` into `src/signals.rs` (`src/signals.rs`, `src/lib.rs`, `src/review.rs`, `src/checkpoint.rs`, `tests/review_skip_and_signals.rs`).
- **Slice 02 — Answered asks, `factory review settled`, and `state.toml` `reviewed_head` (`FR-3b` state/settled + `FR-4` settled, deps: —):** adds optional `reviewed_head` to `state.toml` (cleared with `reviewed=`) and prefers it in `settled::last_reviewed` with `^reviewed =` fallback, includes answered `Kind::Ask` entries in `settled::decisions`, and adds `factory review settled <NNN>` (`src/docs/state.rs`, `src/settled.rs`, `src/cli/review.rs`, `tests/settled_and_state.rs`). Independent of Slice 01 — builds in parallel.
- **Slice 03 — Structured claims index and unanchored `pack::collected` (`FR-2`, deps: 01):** extends `review::diff_prompt` with the `### Summary` table instruction (updating authorized `no_feature_keeps_the_prompt_as_today`), extracts and merges pass tables into `### Claims index` at the top of `## Second opinion`, and updates `pack::collected` to stop copying at any `### ` heading (`src/review.rs`, `src/collect.rs`, `src/pack.rs`, `tests/review.rs`, `tests/review_claims_and_since.rs`).
- **Slice 04 — Unreviewed failure causes, branch-scoped `--since`, and polish skill (`FR-3a`, `FR-3b` collect/closeout, `skills/polish/SKILL.md`, deps: 02, 03):** adds deterministic `review::classify_failure` and safe probe usage-limit detection (updating authorized `collect_owes_the_fallback_when_every_pass_timed_out`), wires `collect::head_of` into `closeout::ship` and branch-intersected `--since` with rebase fallback, and applies all three `skills/polish/SKILL.md` updates + `skills/bundle.lock` regen in one pass (`src/review.rs`, `src/collect.rs`, `src/closeout.rs`, `skills/polish/SKILL.md`, `skills/bundle.lock`, `tests/review.rs`, `tests/review_claims_and_since.rs`).

Checkpoints fall every 2 done slices (`checkpoint_every = 2`: after 01+02, then after 03+04). Workflow: one branch (`feat/review-and-polish-speedup`), one PR, one commit per slice, Prove rows driven at checkpoints. No QA plan row needs a human, so no `qa-guide.md` is generated.

## 10. Non-goals

- **LLM post-pass summarizer for `collect.md`.** Not handled (2026-11-26): adds cost and latency; deterministic table parsing in FR-2 covers the need. Reopen only if reviewer models routinely fail to emit the requested markdown table.
- **3-way range-diff for `--since` on files edited on both `main` and the branch.** Not handled (2026-11-26): path-level intersection with `<base>...HEAD` removes all unrelated `main` files without range-diff complexity.
- **Changing `leaks` scan range.** Not handled (2026-11-26): `leaks` continues scanning `<base>...HEAD` on every collect.

## 11. Verification notes

Checked against `main` at `9bbf37e`, 2026-11-26; line numbers and blast radius corrected 2026-11-26 after cross-vendor adversarial pass (`claude:opus`, log `20261126-235528`). **Re-verify at slice entry.**

- `src/review.rs:241-263` (`resolve_range`, `numstat`), `:266-333` (`Skip`, `skip`, `tool_written`), `:470-529` (`CHILD_GROUPS`, `Watched`, `forward`, `forward_signals`), `:671-739` (`probe_resolved`), `:1322-1467` (`pass_text`, `pass_header`, `diff_pass_prompt`, `body`, `diff_prompt`).
- `src/collect.rs:21-40` (`FALLBACK_OWED`), `:51-56` (`reusable`), `:123-215` (`collect`, `:133-134` range & `review::skip`), `:247-322` (`second_opinion`).
- `src/pack.rs:126-165` (`collected` unanchored local-review rule).
- `src/settled.rs:15-93` (`for_feature`, `block`, `last_reviewed`, `decisions`).
- `src/docs/state.rs:26-114` (`State`, `Raw` with `deny_unknown_fields`, `parse`, `render`), `:165-226` (`apply`, `unknown`).
- `src/closeout.rs:401-445` (`collected`), `:1055-1168` (`ship`).
- `src/cli/review.rs:17-64` (`ReviewVerb`, `SecondArgs`, `CollectArgs`), `:92-222` (`collect_file`, `second`).
- `src/checkpoint.rs:36-73` (`ACTIVE_GATE_PGID`, `forward_gate_signal`, `forward_gate_signals`).
- `skills/polish/SKILL.md:33-35` (attended `--since` & unanchored reading rule), `:65-95` (`second opinion:` line, fallback brief template, `## 2. Verify and dedup`).
- `tests/review.rs:698-814` (`collect_owes_the_fallback_when_every_pass_timed_out`, `collect_skips_tool_written_and_vendored_files_and_lists_them`, `second_skips_the_same_files_and_lists_them_before_the_passes`), `:1016-1046` (`no_feature_keeps_the_prompt_as_today`), `:1076-1174` (`collect_since_*`, `last_reviewed_commit_is_the_one_that_set_reviewed`, `clearing_reviewed_asks_for_the_whole_branch`).
- `tests/skills_bundle.rs:416-436` (`polish_lists_five_settled_sources`).
