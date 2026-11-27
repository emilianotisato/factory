# Slice 01 — Skip feature docs in code passes and unify signal forwarding

**Source**: [`prd.md`](prd.md) → §3 (FR-1) and §6 (FR-4 signal forwarding). **Depends on**: — _(independent of Slice 02; lands before Slice 03 so `src/review.rs` edits do not collide)_
**Created**: 2026-11-26

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Drops `docs/features/**` planning docs (`prd.md`, `slice-*.md`, `retro.md`, `qa-guide.md`) from `review::skip` as `"feature-doc"` whenever the diff includes at least one non-feature-doc file (keeping docs-only branches reviewed in full), and unifies `SIGINT`/`SIGTERM`/`SIGHUP` process-group forwarding between `src/review.rs` and `src/checkpoint.rs` into one shared `src/signals.rs` registry so installing one handler never clobbers the other.

## Current code / environment

- **`review::skip` & `tool_written`**: `src/review.rs:266-333` — `Skip.skipped` (`:269`) lists `tool-written`, `vendored`, or `generated`. `skip()` checks `tool_written(p)` (`log.jsonl` / `state.toml` under `docs/features/<dir>/`) first, then `git check-attr` for `linguist-vendored`/`linguist-generated`.
- **Existing skip tests (NOT authorized to modify)**: `tests/review.rs:728` (`collect_skips_tool_written_and_vendored_files_and_lists_them`) and `:787` (`second_skips_the_same_files_and_lists_them_before_the_passes`) test `LOG` (`log.jsonl`), `STATE` (`state.toml`), `testdata/a.xsd`, and `src/a.rs`. Checking `tool_written` first keeps both tests green untouched!
- **Duplicate signal handlers**:
  - `src/review.rs:470-529` — `CHILD_GROUPS: [AtomicI32; 256]`, `Watched`, `forward`, `forward_signals` (calls `libc::sigaction` under `Once::call_once`).
  - `src/checkpoint.rs:36-73` — `ACTIVE_GATE_PGID: AtomicI32`, `forward_gate_signal`, `forward_gate_signals` (calls `libc::sigaction` under its own `Once::call_once`, overwriting `review.rs`'s handler or vice versa!).
- **Crate root**: `src/lib.rs` exports library modules (`pub mod ...`).
- **Deliberate non-changes** — *(prd §10 Non-goals)*:
  - Do NOT touch `leaks` scan scope (`leaks` continues scanning `<base>...HEAD`).
  - Do NOT modify `tests/review.rs` or `tests/stubs/*` in this slice; put the new integration tests in a new `tests/review_skip_and_signals.rs` file.

## Tasks

### 1. Drop `docs/features/**` planning docs in `review::skip` when code also changed — high — blocked by: —
**Source**: §3 FR-1: "Add `feature_doc(path: &str) -> bool`", "if at least one candidate file is NOT a `feature_doc`, move every `feature_doc` candidate into `out.skipped` with reason `\"feature-doc\"`", "If every remaining candidate file is a `feature_doc`, keep them all in `out.kept`".

`src/review.rs`.
1. Update the doc comment on `Skip.skipped` (`:269`) to list `tool-written`, `feature-doc`, `vendored`, or `generated`.
2. Add `fn feature_doc(path: &str) -> bool { path.starts_with("docs/features/") }`.
3. In `skip(root: &Path, files: Vec<(String, u64)>) -> Skip` (`:299-324`):
   - Keep `tool_written(&path)` skipped first as `"tool-written"` and `git check-attr` matches skipped as `"vendored"` / `"generated"`, collecting the remaining candidates in order.
   - Check `let has_non_feature_doc = candidates.iter().any(|(p, _)| !feature_doc(p));`.
   - If `has_non_feature_doc`, push every `(p, _)` with `feature_doc(&p)` to `out.skipped` with reason `"feature-doc"`, and the rest to `out.kept`.
   - Otherwise (docs-only diff), push all candidates into `out.kept`.

**The trap**: Only drop `feature_doc` paths when there is at least one non-feature-doc file remaining AFTER `tool_written` and `vendored`/`generated` filtering — otherwise a PR that only edits `docs/features/001-x/prd.md` + `testdata/vendored.xsd` would wrongly skip `prd.md` and review nothing!

**Acceptance:** Unit test in `src/review.rs` (plus integration test in Task 3) proves mixed `src/a.rs` + `docs/features/001-x/{prd.md,slice-01.md,log.jsonl}` skips `log.jsonl` as `tool-written`, skips `prd.md`/`slice-01.md` as `feature-doc`, and keeps `src/a.rs`; docs-only `docs/features/001-x/{prd.md,log.jsonl}` keeps `prd.md` (fails if `has_non_feature_doc` is inverted or omitted).

### 2. Unify `SIGINT`/`SIGTERM`/`SIGHUP` forwarding into `src/signals.rs` — high — blocked by: —
**Source**: §6 FR-4: "Create a small shared `src/signals.rs` holding one `SLOTS = 256` `AtomicI32` table (`CHILD_GROUPS`)...", "Replace the duplicate `CHILD_GROUPS`/`forward_signals` in `src/review.rs:470-529` and `ACTIVE_GATE_PGID`/`forward_gate_signals` in `src/checkpoint.rs:36-73`".

`src/signals.rs`, `src/lib.rs`, `src/review.rs`, `src/checkpoint.rs`.
1. Create `src/signals.rs` and declare `pub mod signals;` in `src/lib.rs`:
   - `const SLOTS: usize = 256;` and `static CHILD_GROUPS: [AtomicI32; SLOTS] = [const { AtomicI32::new(0) }; SLOTS];`.
   - `pub struct Guard(&'static AtomicI32);` with `pub fn claim() -> Result<Guard>` (CAS `0 -> -1`), `pub fn set(&self, pgid: i32)` (`store(pgid, Ordering::SeqCst)`), and `Drop` (`store(0, Ordering::SeqCst)`).
   - `pub fn install()` (on `#[cfg(unix)]`, `Once::call_once` registering `SIGINT`, `SIGTERM`, `SIGHUP`, preserving `SIG_IGN`, handler kills every `pgid > 0` with `libc::kill(-pgid, libc::SIGKILL)`, resets `libc::SIG_DFL`, and `libc::raise(sig)`).
2. In `src/review.rs`: delete `SLOTS`, `CHILD_GROUPS`, `Watched`, `forward`, and `forward_signals` (`:470-529`), and in `run_logged` (`:550-559`) call `crate::signals::install(); let watched = crate::signals::Guard::claim()?; ... watched.set(child.id() as i32);`.
3. In `src/checkpoint.rs`: delete `ACTIVE_GATE_PGID`, `forward_gate_signal`, and `forward_gate_signals` (`:36-73`), and wherever the gate child process group is registered, call `crate::signals::install()` and hold a `crate::signals::Guard` (`guard.set(pgid)`) until the gate child finishes (ensuring `Drop` clears the slot on every exit/error path).

**The trap**: Make sure `checkpoint.rs` holds the `Guard` across the entire gate child wait loop (including cancellation/timeout branches) and never drops it early; verify `rg sigaction src/review.rs src/checkpoint.rs` returns zero matches!

**Acceptance:** `src/review.rs` and `src/checkpoint.rs` contain zero `sigaction` calls and both register child groups via `crate::signals`; subprocess test in Task 3 proves one `SIGTERM` kills two registered child process groups at once.

### 3. Add integration tests in `tests/review_skip_and_signals.rs` — high — blocked by: 1, 2
**Source**: §3 FR-1 Acceptance & §6 FR-4 (4) Acceptance.

`tests/review_skip_and_signals.rs` (new file).
1. Add `review_skip_drops_feature_docs_when_code_changed_but_keeps_docs_only_diffs`:
   - Mixed branch (`src/a.rs`, `docs/features/001-x/prd.md`, `docs/features/001-x/slice-01-a.md`, `docs/features/001-x/log.jsonl`): run `factory review collect` (with a temporary stub binary in tempdir) and assert `Plan` lists `prd.md` and `slice-01-a.md` as `(feature-doc)`, `log.jsonl` as `(tool-written)`, and the pass diff contains ONLY `src/a.rs`.
   - Docs-only branch (`docs/features/001-x/prd.md`, `docs/features/001-x/slice-01-a.md`, `docs/features/001-x/log.jsonl`): assert `log.jsonl` is `(tool-written)` while `prd.md` and `slice-01-a.md` ARE kept and sent to the pass diff.
2. Add `signals_unified_kills_both_child_groups`:
   - Re-execute `std::env::current_exe()` with `--exact signals_unified_kills_both_child_groups` and env `FACTORY_SIGNALS_TEST_CHILD=1`.
   - In the child branch (`FACTORY_SIGNALS_TEST_CHILD=1`): call `factory::signals::install()`, claim two `Guard`s, spawn two `sleep 60` children with `.process_group(0)`, register both pids with `guard.set(pid)`, write both pids to a ready-file path passed via env, and sleep until signaled.
   - In the parent branch: wait for the ready-file with the two grandchild pids, send `SIGTERM` to the child process, wait for the child to exit on `SIGTERM`, and assert BOTH grandchild process groups are dead (`libc::kill(pid, 0) != 0` within a short poll window).
   - Also assert neither `src/review.rs` nor `src/checkpoint.rs` contains `"sigaction"`.

**Acceptance:** `cargo test --test review_skip_and_signals` passes; removing `feature_doc` filtering or dropping either `Guard` registration causes the test to fail.

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §8 QA plan FR-1 row (and FR-4 signal-unification half).
- **FR-1**: run `cargo test --test review_skip_and_signals review_skip_drops_feature_docs_when_code_changed_but_keeps_docs_only_diffs -- --nocapture` and keep the transcript. Expected: passes proving `(feature-doc)` skipping on mixed branches and full review on docs-only branches.
- **FR-4 (signals)**: run `cargo test --test review_skip_and_signals signals_unified_kills_both_child_groups -- --nocapture` and keep the transcript. Expected: passes proving one `SIGTERM` kills two active child process groups and zero `sigaction` remains in `review.rs`/`checkpoint.rs`.

**Acceptance:** after the checkpoint, the FR-1 and FR-4 (signals) rows have transcript evidence in this slice's **Progress**.
