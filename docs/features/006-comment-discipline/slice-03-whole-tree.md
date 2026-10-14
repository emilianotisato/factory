# Slice 03 — Tuning, the allowlist, doctor and preflight

**Source**: [`prd.md`](prd.md) → §5 FR-3 (the tuning against three repos, the allowlist entries, "Where it runs" item 3; its third and fourth acceptance bullets), §7 FR-5 (the false positives, the run's proof on the feature branch). **Depends on**: 01 (it tunes the detectors slice 01 wrote, and the doctor and preflight lines call its `scan`)
**Created**: 2026-10-14

**Progress**:
- **Built.** `src/leaks.rs` gained `QUIET`, a table of shapes blanked out of the text before a named rule runs. It also gained a three-or-more-digit `cp` skip in `name_hit`, a whole `slice<n>` identifier skip in `.go` files (`go_slice`), and a two-digit-only `L` and no `S3` in the `finding-id` row. "no longer" left the `history` row (AD-3); the other trades are AD-4 and AD-5. `leaks::verdict(root)` scans `--all` and returns `clean`, `N hits — run factory leaks --all and clean before the first run`, or `unknown: <error>`. `factory docs doctor` prints it as the `leaks` line after `drift` and its detail lines. The preflight puts `leaks clean` in `ok` and anything else in `judge`.
- **Factory's own tree.** The test locals `ad1`, `ad2`, `ad3` and `b1` in `src/docs/log.rs` are now `lock`, `helper`, `ask` and `red`. `AD1`, `AD2`, `ask1`, `block1`, `answer1` and `answer4` in `src/docs/check.rs` are now `FIRST_DECISION`, `SECOND_DECISION`, `ask_out_of_order`, `block`, `answer` and `stray_answer`. `slice_01` and `slice_02` in `tests/lanes.rs` are now `first` and `dependency`. `docs/project.toml` carries `[leaks] allow`: the six `all` entries the PRD names, then 45 `strings` entries. Each `strings` file's only hits are id-shaped or `file:line` strings that are test data or the tool's own format text; `src/docs/migrate.rs:846` shows the FR id format in the migrate output. No multi-line literal in `src/leaks.rs` or `tests/leaks.rs` read as a comment, so both are `strings`.
- **Evidence.** `docs/features/006-comment-discipline/evidence/leaks-cody-agent.txt` (2172 hits), `leaks-arca-api.txt` (1280 hits) and `leaks-factory.txt` (clean). Each has the repo's `HEAD` on line 1. The factory report scans this slice's uncommitted tree from a scratch commit, because `--all` reads `HEAD`; the checkpoint reruns it on the commit. `factory docs doctor` on that scratch commit printed `leaks          clean`.
- **Verdicts, by rule and shape.** Every hit left is real.
  - arca, `id-comment`: an uppercase plan id (`FR-n`, `PRD-n`, `AD-n`, `OPEN-n`, `DEC-n`), 704. A slice number (`(slice 2)`, `Slice 13:`), 62. A `§` cite into a PRD or a repo doc (`prd §10`, `architecture/…md § Cooldown`, `agent-log § Traps`), 72.
  - arca, `id-name`: a test named after an FR (`TestFR4…`, `TestProveFR5…`), 19. A function named after a slice (`slice2Allowlists`…`slice12Allowlists`, `TestWriterAcceptsSlice3…`), 9. `FR43_ROWS`, 1. The path `internal/caea/informe_prove_fr45_integration_test.go`, 1.
  - arca, `id-string`: a slice number in a console log, screenshot or run label (`[slice 17]`, `slice05 ${RUN_ID}`, `Org Slice 3`), 113. An uppercase plan id (`CP-3 fleet`, `(AD-26)`), 111. A lowercase plan id (`prd-016-capture`, `fr-4a-…` evidence names), 26.
  - arca, `history`: "used to" in its past sense, 41. "Confirmed on prod DOKS", "a code added in", 2. "before PRD-013", 1. "were previously guillotined", 1.
  - arca, `section-cite`: `prd §n`, 57. `<file>.md §`, 15. `architecture/31 § 5` in a `t.Logf`, 2.
  - arca, `step-marker`: `Task n` and `--- task n:` beside a slice or FR, 28. `line-cite`: a `file:line` comment, 3. `finding-id`: the scenario ids `S0`, `S6` and `S11`, 12.
  - cody, `id-comment`: an uppercase plan id, 1473. A slice number (`slice 04`, `slice-02`, `Slice 6.3`), 88. A bare `§3.3` PRD cite or a `--- §1` banner, 267.
  - cody, `id-string`: an uppercase plan id (test titles with `(AD-10002)` and `(DEC-10)`), 128. Seeder lines with `slice 05`, 9. `%PDF-1.1 fr-158`, 3.
  - cody, `history`: "used to" in its past sense, 44. "until slice NN" and "before PRD-0NN", 10. "previously" in its past sense, 5. "for now" as a temporary state, 2. "confirmed on" above a TODO tied to feature 007, 1.
  - cody, `section-cite`: `PRD §n`, 106. `step-marker`: `task n` beside a slice or FR, 28. `line-cite`: a `file:line` comment (`Process.php:336,367`, `engine.ts:14`), 8.
- **Tuned away**, each now a unit-test non-hit beside the example that must still hit:
  - "no longer" as a state, about 45 (cody `app/…` and arca `internal/…`, e.g. "a row that is no longer pending").
  - "used to" as purpose, about 55. Examples: cody `PasswordValidationRules.php:11,21` and `ProfileValidationRules.php:12,25,35` ("the validation rules used to validate"). Arca `caea/informe.go:126` ("the CAE path used to issue"), `health/state_store.go:281`, `wsaa/types.go:39`, `signing/parser.go:50`, `http/manage_grant.go:201`, `padroningest/runner.go:30`, `spaces/spaces.go:2` and `caea/informe_backoff.go:477`.
  - "previously" as an adjective: cody `BuildSoulTest.php:97` and `GatedDownloadTest.php:162`.
  - "for now and": arca `padroningest/schedule.go:42`.
  - `L7`: arca `infra/k8s/cilium-arca-ingest-egress.yaml:32` and `deployment-api.yaml:101`.
  - `S3`: arca `internal/spaces/spaces.go:1` and cody `LeafAssetFetcher.php:13`.
  - `§` after a standard or a manual: cody `tests/Pest.php:1010` (RFC 7636), and arca `wsaa/cache.go:122,216` and `fiscal/money.go:11` (WSAA and WSFEX manuals).
  - A package version: cody `package-lock.json:1985,3626`.
  - Factory's `ad1`…`ad3`, `AD1`, `AD2` and `slice_01` were real leaks, renamed rather than tuned.
- **Tests.** The unit tests in `src/leaks.rs` cover:
  - No hit on `slice1` in a `.go` file, on `cp1252`, on L7, S3, RFC and manual `§`, a package version, or the purpose and adjective senses.
  - A hit on `prd_003`, `tests/fixture/prd-004`, `fr157-switch.png`, `slice2Allowlists` (in .ts and .go), `section_14`, `cp12_resume`, `FR-1.2`, and the past senses.
  - One integration test in `tests/leaks.rs`: on the clean fixture, `docs doctor` prints `leaks          clean` after `drift`, and the preflight prints `  ✓ leaks clean`. After two seeded leaks, doctor prints `leaks          2 hits — run factory leaks --all and clean before the first run`, and the preflight prints `  ? leaks 2 hits — …` with the same exit code.
- **Inner loop.** 1 run, 0 red, about 1.5 minutes. Before it, a targeted `cargo test --test leaks` went red once. Preflight refuses a branch not named for the feature, so the seeded test commits on `feat/fixture`.

**Slice**: After this slice `factory leaks --all` reports no false hit over cody-agent, arca-api and factory's own tree, factory's `docs/project.toml` carries its allowlist, and `factory docs doctor` and `factory run`'s preflight each print one `leaks` line from the same function: the hit count with the command to run, or `clean`. It warns and never blocks.

**Highest risk**: task 1. The name pattern joins ids "by `-`, `_` or nothing", so Go's `slice1`, the `cp1252` codepage and factory's own `ad1` read like leaks by shape alone.

## Current code / environment

- **The repos to tune against**, read-only: `~/Projects/CODESM/cody-agent` (Laravel and Vue) and `~/Projects/Prokey/arca-api` (Go, 552 bare ids in its code per §7), and the lane itself. The verb finds the repo from the current directory (`paths::repo_root(Path::new("."))`), so run the lane's build with the other repo as the working directory: `cd ~/Projects/Prokey/arca-api && cargo run -q --manifest-path <lane>/Cargo.toml -- leaks --all`. `--all` reads `HEAD`'s committed tree only and writes nothing. Never edit, commit or check out anything in those two repos *(prd §11 "Not handled: cleaning cody or arca in this PRD." The owner hands them the report later)*.
- **The collisions the PRD names** (§5 Accepted cost): Go's `slice1`, the `cp1252` codepage, and factory's test locals `ad1`, `ad2` (`ad3` too) in the test modules of `src/docs/log.rs` and `src/docs/check.rs`. `SLICE_01` and `SLICE_02` in `tests/merge.rs` sit in a file the PRD lists as `all`, so they need nothing.
- **The false positives FR-5 found** (§7): "asset names (`the_prd_scaffold_frontmatter_has_the_seven_keys` names `prd.scaffold.md`), schema and store versions (`v2`, `version_3_store`), gate kinds (`"checkpoint"`, `"slice"`), `PRD-{feature}` format strings, a skill's own section number inside a string (polish's `§5`), and the fixture's slice numbers in test comments." Slice 01's unit tests hold most of these; this slice adds what the three real trees turn up.
- **Factory's id-shaped data**: §5 "about 400 non-comment Rust lines hold id-shaped strings today (`tests/lanes.rs` alone has 81)". The only id-shaped path outside `docs/` is `tests/fixture/v2/docs/features/001-fixture/`, which the PRD lists as `all`. Slice 01's own `src/leaks.rs` and `tests/leaks.rs` hold the rule examples as string data.
- **Doctor**: `src/docs/doctor.rs:53` `run` builds lines with `line(key, value)` (`:49`, `{key:<15}{value}`); the v3 branch (`:58-74`) pushes `gate`, `skills`, then `drift` and, when drift is not clean, its detail lines. Its one caller is the `doctor` verb (`src/cli/docs.rs:308`).
- **Preflight**: `src/docs/preflight.rs:87` `run` sorts lines into `refuse`, `ask`, `judge` and `ok`; the v3 block (`:95-116`) ends with the skills verdict: `let into = if v.is_ok() { &mut ok } else { &mut judge };` (`:113-114`). `cli` (`:349-383`) prints them as `✓ ✗ ! ?`; `factory run` prints each judge line as `  · {text}` (`src/cli/run.rs:136`) and records a `judge_noted` event per line.
- **Tests that pin the order and the counts**:
  - `tests/skills_fence.rs:203` asserts doctor's first four keys are `["Framework", "gate", "skills", "drift"]`: the `leaks` line goes after `drift` and its detail lines.
  - `tests/docs_check.rs:67` asserts `"drift          clean\n"` on the clean fixture.
  - `tests/run.rs:347` and `tests/loop.rs:572` count `judge_noted` events on the clean fixture. The fixture's files (`tests/fixture/gate.sh`, `src/lib.txt`, written by `tests/harness/mod.rs:195-233`) hold no hit today, so a `leaks` line that goes to `judge` only on hits leaves those counts alone. Keep it that way.
- **Deliberate non-changes**: *(prd §11 "Not handled: refusing to run on a base with leaks.")* The line warns; it is never a `refuse` or an `ask`. *(prd §11 "Not handled: a per-repo rule table".)* A collision is tuned in the embedded table or allowlisted by path, never configured per repo.

## Tasks

### 1. Tune the detectors against three trees — high — blocked by: —
**Where**: `src/leaks.rs` (detectors and their unit tests); `docs/features/006-comment-discipline/evidence/leaks-cody-agent.txt`, `leaks-arca-api.txt`, `leaks-factory.txt`.

**Source**: §5 FR-3 Accepted cost: "Short forms collide with real names: Go's `slice1`, the `cp1252` codepage, and factory's own test locals that hold a fixture's log entries (`ad1`, `ad2` in `src/docs/log.rs` and `src/docs/check.rs`). So the pattern is tuned against all three repos before it ships." Acceptance: "A read-only `--all` run over the whole of cody-agent … and arca-api …, and over the feature branch, reports no false hit."

Run `--all` in each of the three trees and save each full report, with the repo's `HEAD` sha on its first line, as the evidence file above. Read every hit. A hit that is not a real leak is a false hit: change the detector so it no longer fires, and add the case to the row's unit tests as a non-hit beside the PRD example that must still hit. Rerun until the three reports hold only real leaks. Then group the hits of each report by rule and by shape (e.g. "arca, `id-comment`, `// FR-n:` above a handler: 212, real"), with a verdict per group and every false hit you tuned away named on its own line.

How a shape is told apart is the builder's call, logged as a decision when it trades a real leak for a quiet one. Both readings are open: `slice1` as a whole identifier in a `.go` file is Go's slice type, `slice2Allowlists` is a leak; `cp1252` has four digits, a checkpoint id has one or two.

**The trap**: the hit count in arca is in the hundreds, and every one of the 552 bare ids §7 counted is real. Tuning for a smaller number is the wrong goal: a false hit is the only thing to remove.

**Acceptance:** unit tests in `src/leaks.rs`: no hit on `slice1` in a `.go` file and on `cp1252`; a hit on each of the five name examples (`prd_003`, `tests/fixture/prd-004`, an `frNNN-` screenshot name, `slice2Allowlists`, `section_14`). The three evidence files exist, and the grouped verdicts are in hand for task 6.

### 2. Settle factory's own collisions — high — blocked by: 1
**Where**: the test modules of `src/docs/log.rs` and `src/docs/check.rs`.

**Source**: §5 FR-3 Accepted cost, above, and FR-1's rule (§3): "Name the behavior … Do not copy what is nearby when it breaks this. Fix it when you touch it."

`ad1`, `ad2` and `ad3` name a fixture's log entries by their ids, which is the shape the `id-name` row exists to catch. Rename them for what they hold (e.g. `chose`, `revised`, `answered`, whatever the test reads them as). `SLICE_01` and `SLICE_02` stay: `tests/merge.rs` is an `all` entry (task 3).

**Acceptance:** `factory leaks --all` on the lane reports neither file; the inner loop is green.

### 3. Factory's allowlist — high — blocked by: 1
**Where**: `docs/project.toml`.

**Source**: §5 FR-3: "factory lists its four scenario test files and its recorded fixtures as `all`" (§7), naming "`tests/checkpoints.rs:481,658,743,1368,1422`, `tests/closeout.rs:937`, `tests/merge.rs:96`, `tests/quota.rs:377`" and "recorded output and fixtures (`tests/stubs/cursor-review.jsonl`, `tests/fixture/v2/`)". And: "`strings` (the default) turns off the string check under the path and nothing else … An `all` entry hides a future leak under it, so the list stays that short."

Write the `[leaks] allow` table slice 01 reads: the six `all` entries above, then one `strings` entry per file whose only hits are id-shaped strings that are data (strings compared with the tool's output or a fixture, as §7 says: "Ids as data are decided by location, not by shape"). `src/leaks.rs` and `tests/leaks.rs` are `strings` entries; if a multi-line literal in either reads as a comment, make that file `all` and log the decision. Every other hit is fixed in the code, not listed.

**Acceptance:** task 6's run.

### 4. The doctor line — high — blocked by: 3
**Where**: `src/leaks.rs` (one function for the line), `src/docs/doctor.rs:58-74`.

**Source**: §5 FR-3, "Where it runs" item 3: "`factory docs doctor` prints one more line from `--all` on the tree: `leaks    N hits — run factory leaks --all and clean before the first run`, or `leaks    clean`, and `factory run`'s preflight (`src/docs/preflight.rs:98`) prints the same line from the same function. It warns and never blocks, like drift."

One function in `src/leaks.rs` scans `--all` at the repo root and returns the line's value: `N hits — run factory leaks --all and clean before the first run`, or `clean`, or `unknown: <error>` when the scan fails (the doctor's existing form for a check it could not run, `src/docs/doctor.rs:76`). Doctor pushes it as `line("leaks", …)` in the v3 branch, after `drift` and its detail lines.

**Acceptance:** an integration test: `factory docs doctor` on a fixture branch holding two seeded leaks prints `leaks          2 hits — run factory leaks --all and clean before the first run`; on the clean fixture it prints `leaks          clean`; `tests/skills_fence.rs:203`'s first four keys hold.

### 5. The preflight line — high — blocked by: 4
**Where**: `src/docs/preflight.rs:95-116`.

**Source**: as task 4: "`factory run`'s preflight … prints the same line from the same function. It warns and never blocks."

In the v3 block, after the skills verdict, call task 4's function: `clean` goes to `ok` as `leaks clean`, hits and `unknown` go to `judge` as `leaks …`. Never to `refuse` or `ask`.

**Acceptance:** an integration test: `factory docs preflight` on the seeded fixture branch prints `  ? leaks 2 hits — …` and exits as it did before the leaks; on the clean fixture `  ✓ leaks clean`. `tests/run.rs:347` and `tests/loop.rs:572` pass unchanged.

### 6. Verify — medium — blocked by: 1, 2, 3, 4, 5
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) §9 this slice proves, below the UI:

- **FR-3**, its second half: "Then run it read-only with `--all` over `~/Projects/CODESM/cody-agent`, `~/Projects/Prokey/arca-api` and the feature branch, and write every hit with its verdict into the slice's log entry". Every hit is in the three evidence files from task 1, rerun at the checkpoint against the lane's build. The verdicts, grouped by rule and shape with each group's count and every tuned-away case named, go into the checkpoint's log entry under **Prove**, pointing at the files: arca's hundreds of real ids do not fit one entry line by line.
- **FR-3**, the doctor and preflight part of row b: "Run `factory docs doctor` and `factory docs preflight` on the seeded branch: the `leaks` line shows the count" — tasks 4 and 5's tests.
- **FR-5**: "run `factory leaks --all` on the feature branch, which holds the merged cleanup: zero `fix` hits outside the allowlist, and `factory docs doctor` prints `leaks    clean`; the gate is green" — run on the lane at this checkpoint. Slices 04 and 05 name it again, so the final checkpoint reruns it on the tree that ships whichever of the three lands last.

**Acceptance:** after the checkpoint, Progress holds the three evidence paths, the doctor and preflight test transcript, and the `--all` and `doctor` output on the lane.
