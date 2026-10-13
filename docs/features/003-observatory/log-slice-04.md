## Traps

- The driver commits a shared file whole, with whichever slice lands first. This slice's hunks in `src/db.rs`, `src/cli/report.rs` and `tests/stubs/claude` went into slice 02's commit `bb933ab`. Read `git log --stat` before you list what your commit holds.
- A run parked on a blocked worker never returns when resumed with `--foreground`: the driver waits for the answer. A test that re-runs a parked run should park it on a preflight ask (an unchained fixture, no `--foreground`).
- The harness index declares `lanes: 2`. Pass `("lanes", "1")` to `Fixture::with_index` when a test asserts the lane count.

## Decisions

### AD-401 — How does the knobs text show a role whose effort came from another scope? · slice-04 · 2026-10-12
**Chose:** `polish sonnet/high project/default`: the model's scope, then `/` and the effort's scope, only when the two differ. In the fixture, `docs/project.toml` sets only the polish model, so its effort comes from the default and the line reads `project/default`.
**Over:** the slice's example `polish sonnet/high project`, which would hide the `effort_source` the JSON carries.
**Because:** task 2 writes `effort_source` exactly when the scopes differ, and the text should not drop what the row says. The acceptance check `polish sonnet/high project` still matches as a prefix.
**Undo:** drop the `effort_source` branch in `knobs_text` (`src/cli/report.rs`).
**Review:** fyi

### AD-402 — Which resume does the `--hypothesis` harness test drive? · slice-04 · 2026-10-12
**Chose:** a run parked on a preflight ask, whose driver has exited: the second `factory run 001 --hypothesis other` takes the resumable branch. The live branch calls the same `hypothesis_ignored` helper.
**Over:** a live run with its driver still running, as the acceptance words it, which needs a slow worker and a pid trick to hold the run open.
**Because:** a parked-on-a-block run resumed with `--foreground` waits for the owner and hangs the test (see Traps). The refusal line and the unchanged row are the same on both branches.
**Undo:** add a live-run case using the `set_pid` pattern from `tests/run.rs`.
**Review:** fyi

### AD-403 — Who adds slice 03's `pack_by_role` to the key list in `tests/metrics.rs`? · slice-04 · 2026-10-12
**Chose:** this slice added it after `knobs` and `hypothesis`. Slice 03 was already done and committed without it, so `report_prints_the_retro_keys_then_the_run_keys` was red on the shared tree.
**Over:** leaving the test red for the checkpoint to fix.
**Because:** the list must mirror `EXTRA_KEYS` in `src/cli/report.rs`, which already ends `knobs, hypothesis, pack_by_role`. It is a one-line addition, and nothing is weakened.
**Undo:** remove the `"pack_by_role"` line.
**Review:** no
