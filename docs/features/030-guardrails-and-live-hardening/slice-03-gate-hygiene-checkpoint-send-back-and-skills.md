# Slice 03 — Gate hygiene, checkpoint send-back, and skills verification rules

**Source**: [`prd.md`](prd.md) → §5 (FR-3). **Depends on**: — _(independent of Slices 01, 02, 04, 05)_
**Created**: 2026-11-27

**Progress**: `isCollapsed` + `retry_busy`/`spawn_retry_busy`, `SentBack` pack section, `evidence::rotate_stale_evidence`, three skill phrases + `skills/bundle.lock`, `tests/checkpoint_send_back.rs`, `src/mods.rs` embed comment. Inner loop: 6 runs, 3 red, ~28 min — stale `include_str!`/incremental rmeta for cockpit embed and new checkpoint symbols; plan `step("checkpoint")` masked second short record; clippy `retry_busy` needs `Result<i32>` annotation. Deliberate red: `retry_busy` single-shot → `ExecutableFileBusy`; omit `# Sent back` → `sent_back_section_names_missing_slices_and_every_row`; skip rotation in `checkpoint::run` → `checkpoint_send_back_tells_respawn_why_and_rotates_stale_evidence`.

**Slice**: Clears the 2 `TS2345` warnings from `claude plugin validate mods/cockpit`, fixes the `ETXTBSY` race in `provider::discover_models` with bounded `ExecutableFileBusy` spawn retries, gives sent-back checkpoint workers a `# Sent back` section naming missing slices/unproven rows while rotating stale `cp-{n}/` evidence to a collision-free `cp-{n}.sent-back-{k}/`, and adds external-platform/live-CLI/third-party-shim verification rules to `/prd`, `/slice`, and `/polish` (+ regenerates `skills/bundle.lock`).

## Current code / environment

- **`mods/cockpit/tests/cockpit.test.tsx`**: lines 11-22 (`BAND.props`) omit `isCollapsed: false`, causing 2 `TS2345` warnings at `:154` and `:168` on `claude plugin validate mods/cockpit`. File is **authorized in `prd.md`**.
- **`provider::discover_models` & `script`**: `src/provider.rs:266-315` spawns CLI probe once with no `ExecutableFileBusy` retry; test helper `script` (`:508-582`) writes scripts directly in place.
- **`Pack::for_checkpoint` & `checkpoint::run`**:
  - `src/pack.rs:427` (`Pack::for_checkpoint`, called in `src/checkpoint.rs:304` and unit tests at `src/pack.rs:1237, 1301, 1484, 1711`).
  - `src/checkpoint.rs:304-465` computes `missing` slices and `unproven` rows on send-back (`:408-420`), then resets the lane via `scheduler::prepare_lane(run, lane, true, out)?` (`:319`) while leaving `$FACTORY_EVIDENCE/cp-{n}/` on disk! Note that attempt counters reset to 1 after a block is answered (`tests/checkpoints.rs:1624`), so rotation MUST pick the first unused `cp-{n}.sent-back-{k}/` (`k = 1, 2, ...`).
- **`skills/{prd,slice,polish}/SKILL.md` & `skills/bundle.lock`**:
  - `v0.18.0` is untagged, so `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` regenerates `skills/bundle.lock` without bumping `Cargo.toml`.
  - Keep all existing passages asserted in `tests/skills_bundle.rs` intact!
- **Deliberate non-changes** — *(prd §11 & `testguard`)*:
  - Do NOT modify `docs/project.toml` (`claude plugin test mods/cockpit` already passes live).
  - Do NOT modify `tests/stubs/claude` (guarded by `testguard`); prove stale evidence non-reuse cleanly in new `tests/checkpoint_send_back.rs` without touching `tests/stubs/claude`.

## Tasks

### 1. Fix `mods/cockpit` `TS2345` warnings and `provider::discover_models` `ETXTBSY` retry — high — blocked by: —
**Source**: §5 FR-3: "Add `isCollapsed: false,` to `BAND.props` in `mods/cockpit/tests/cockpit.test.tsx:14-21`", "Extract a small `spawn_retry_busy(cmd: &mut Command) -> std::io::Result<Child>` helper in `src/provider.rs`".

`mods/cockpit/tests/cockpit.test.tsx`, `src/provider.rs`.
1. In `mods/cockpit/tests/cockpit.test.tsx` (`BAND.props`, `:14-21`, authorized in `prd.md`): add `isCollapsed: false,` so `claude plugin validate mods/cockpit` prints zero `TS2345` warnings and `claude plugin test mods/cockpit` passes all 14 tests.
2. In `src/provider.rs`:
   - Add `fn spawn_retry_busy(cmd: &mut Command) -> std::io::Result<std::process::Child>` (or generic retry helper `retry_busy<T>(f: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T>` that retries `ErrorKind::ExecutableFileBusy` up to 5 times with `std::thread::sleep(Duration::from_millis(10))`), and use it in `discover_models` (`:266-315`) instead of raw `cmd.spawn()`.
   - Update test helper `script` to write to a sibling `.tmp` file, set `0o755`, drop the handle, and `fs::rename` into place.
   - Add a unit test in `src/provider.rs` proving `retry_busy` retries `ErrorKind::ExecutableFileBusy` and succeeds when a subsequent attempt clears (and fails immediately on non-busy errors like `ErrorKind::NotFound`).

**Acceptance:** `claude plugin validate mods/cockpit` prints zero `TS2345` warnings, `claude plugin test mods/cockpit` passes, and `src/provider.rs` unit test proves `ExecutableFileBusy` retry works (fails if retry is removed).

### 2. Add `# Sent back` section to `Pack::for_checkpoint` and collision-free `cp-{n}.sent-back-{k}/` rotation in `checkpoint::run` — high — blocked by: —
**Source**: §5 FR-3: "Add `pub struct SentBack { ... }` and extend `Pack::for_checkpoint`", "rotate `$FACTORY_EVIDENCE/cp-{n}/` to the first unused sibling path `$FACTORY_EVIDENCE/cp-{n}.sent-back-{k}/`".

`src/pack.rs`, `src/checkpoint.rs`.
1. In `src/pack.rs`:
   - Define `#[derive(Debug, Clone, PartialEq, Eq)] pub struct SentBack { pub attempt: u32, pub missing_slices: Vec<u32>, pub unproven_rows: Vec<String>, pub prior_record: Option<u32> }`.
   - Extend `Pack::for_checkpoint` (`:427`) with `sent_back: Option<&SentBack>` (updating unit test call sites at `:1237, 1301, 1484, 1711` to pass `None`).
   - When `Some(sb)`: render a `# Sent back` section naming the prior record (`CP-{k}` if any), missing slices (formatted like `02`), and previously unproven rows, AND explicitly telling the respawn: *"Prior evidence in `cp-{n}/` was moved aside because the lane was reset; re-drive and save fresh evidence for EVERY row of checkpoint {n}."*
2. In `src/checkpoint.rs`:
   - Add `pub fn rotate_stale_evidence(evidence: &Path, n: u32) -> Result<Option<PathBuf>>`: if `evidence.join(format!("cp-{n}"))` exists, loop `k = 1..` until `evidence.join(format!("cp-{n}.sent-back-{k}"))` does not exist, rename `cp-{n}` to that path, and return `Ok(Some(dest))`; if `cp-{n}` does not exist, return `Ok(None)`.
   - In `checkpoint::run` (`:301-465`): keep `let mut sent_back: Option<pack::SentBack> = None;` across the loop, pass `sent_back.as_ref()` to `Pack::for_checkpoint(...)`, and before the send-back `continue`:
     - Populate `sent_back = Some(pack::SentBack { attempt, missing_slices: missing.clone(), unproven_rows: unproven.clone(), prior_record: Some(thin.n) })`.
     - Call `rotate_stale_evidence(&evidence, n)?;`.

**The trap**: Do NOT tell the respawn to only re-drive "previously unproven rows" — because `cp-{n}/` was moved aside, EVERY row of checkpoint `n` needs fresh evidence on the reset lane! And NEVER hardcode `cp-{n}.sent-back-{attempt}` because `attempt` resets to 1 after a block!

**Acceptance:** Unit tests in `src/pack.rs` and `src/checkpoint.rs` (plus integration test in Task 4) prove `# Sent back` names missing slices/unproven rows and instructs re-driving every row, and `rotate_stale_evidence` produces `cp-1.sent-back-1` then `cp-1.sent-back-2` without colliding.

### 3. Add verification rules to `/prd`, `/slice`, and `/polish` and regenerate `skills/bundle.lock` — high — blocked by: —
**Source**: §5 FR-3: "Skills verification rules (`skills/prd/SKILL.md`, `skills/slice/SKILL.md`, `skills/polish/SKILL.md`, `skills/bundle.lock`)".

`skills/prd/SKILL.md`, `skills/slice/SKILL.md`, `skills/polish/SKILL.md`, `skills/bundle.lock`.
1. `skills/prd/SKILL.md` §4 (Adversarial pass): add one sentence containing the exact unique phrase `external platform behavior` instructing the adversarial pass to verify external platform behaviors an FR relies on (e.g. CI trigger rules, webhook/API guarantees), not only repo `file:line` claims.
2. `skills/slice/SKILL.md` §3 (Recon): add one bullet containing the exact unique phrase `verify external CLI flags against the live binary` instructing recon to check external CLI flags against `--help` or official docs before specifying them in a task.
3. `skills/polish/SKILL.md` §1 (`security-review` brief): add one sentence containing the exact unique phrase `never invoke third-party CLIs or lazy shims not under review` forbidding the security subagent from running unreviewed third-party CLIs or lazy shims.
4. Regenerate `skills/bundle.lock` via `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` (do NOT hand-edit `skills/bundle.lock`).

**The trap**: Each of the three phrases must occur **exactly once** in its respective `SKILL.md`, and all existing assertions in `tests/skills_bundle.rs` must stay green!

**Acceptance:** `cargo test --test skills_bundle` passes; each of the 3 exact phrases occurs once in its `SKILL.md` (fails if deleted, duplicated, or `skills/bundle.lock` is stale).

### 4. Add integration test in `tests/checkpoint_send_back.rs` — high — blocked by: 1, 2, 3
**Source**: §5 FR-3 Acceptance & §9 QA plan FR-3 row.

`tests/checkpoint_send_back.rs` (new file — keep free of plan-id strings in comments/names so `factory leaks` stays clean!).
Add `checkpoint_send_back_tells_respawn_why_and_rotates_stale_evidence`:
1. Drive a fixture checkpoint where attempt 1 writes a short checkpoint record covering only slice 1 (leaving out slice 2) AND creates `evidence/cp-1/FR-1/proof.txt`, and attempt 2 writes a full checkpoint record covering slices 1 and 2 WITHOUT creating `evidence/cp-1/FR-1/` (so the test proves stale `cp-1/FR-1/` was NOT retained for attempt 2 — checkpoint detects `FR-1` as unproven and blocks, with zero changes to `tests/stubs/claude`!).
2. Assert `evidence/cp-1.sent-back-1/FR-1/proof.txt` exists (rotated before respawn), assert attempt 2's saved pack contains `# Sent back`, slice `02`, and the instruction to save fresh evidence for every row, and assert calling `rotate_stale_evidence` again when a new `cp-1/` exists rotates cleanly to `cp-1.sent-back-2/` without colliding!
3. Assert each of the 3 exact skill phrases (`external platform behavior` in `skills/prd/SKILL.md`, `verify external CLI flags against the live binary` in `skills/slice/SKILL.md`, `never invoke third-party CLIs or lazy shims not under review` in `skills/polish/SKILL.md`) occurs exactly once.

**Acceptance:** `cargo test --test checkpoint_send_back` passes; removing rotation or `# Sent back` makes the test fail.

### 5. Verify — medium — blocked by: 4
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, `claude plugin validate mods/cockpit` has zero `TS2345` warnings, `claude plugin test mods/cockpit` passes, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
**Source**: §9 QA plan FR-3 row.
- **FR-3**: run `cargo test --test checkpoint_send_back checkpoint_send_back_tells_respawn_why_and_rotates_stale_evidence -- --nocapture && claude plugin validate mods/cockpit` and keep the transcript. Expected: passes proving send-back pack + rotation + skill phrases + zero `TS2345` warnings.

**Acceptance:** after the checkpoint, the FR-3 row has transcript evidence in this slice's **Progress**.
