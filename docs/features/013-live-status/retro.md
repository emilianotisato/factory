# Retro — PRD-013

Written by `/deliver` in the closeout commit, from the agent's side of the run.

**The point is cost next to catch.** An agent asked only about friction votes to cut every check, because every check costs it time. A verdict with an empty Caught column is a preference, not evidence.

## Cost vs catch

| Step | Cost | Caught | Verdict |
|---|---|---|---|
| Inner loop, per slice | one green run per slice, as the slices report it | nothing red recorded | keep: it is the slice's only proof before its commit |
| Checkpoints (full gate) | one run each at CP-1 and CP-2, about a minute and a half each | nothing | keep: cheap, and the plugin half (`claude plugin validate`/`test`) runs only here and in CI |
| Prove, at checkpoints | one transcript per FR row | nothing broken; it produced the FR-1..FR-6 evidence | keep |
| Polish — local review | one forked `code-review` pass | the `factory` marketplace matched by name alone (the install defect that mattered), an unhandled rejection in the poll, a misleading `--here` empty message, a mislabelled parse error | keep |
| Polish — security pass | skipped | nothing to catch: no auth, no input-built paths or shell; the install writes only under the data dir and runs `claude` with fixed argv | keep the rule that skips it here |
| Polish — UX pass | skipped: no `verify-*` harness drives a Claude Code mod | the mod's UI was checked only through `claude plugin test` and the owner's QA row | tune: a cockpit mod has no harness road; the Human row is the only real look |
| Polish — second opinion (grok-4.7-high) | 4 passes, the longest step of polish by far | agreed independently on the marketplace defect; alone found the project-scope plugin match, the missing `immediate`, two `qa-guide.md` steps that fail a correct build, a seconds race in a test, the unpinned version copy in the mod's tests, three weak install-test assertions. About half its claims were rebutted (settled by FR-2, FR-3, AD-3, or a misread of the state API) | keep |
| prune | one PR-mode pass over the scoped tests, run twice more for flakes | nothing to remove; the one flake (board seconds) was already fixed by the review | keep, but it adds little on a branch whose tests were all written by one run |
| Extracting the plugin types from the `claude` binary | a large share of slice-02's time (see Notes) | it is what made `tsc` and the API checks in polish possible | tune: see What I wished existed |

## Where I guessed

- `prd.md` §7 FR-5 step 2 and 3: "unless `claude plugin marketplace list` already shows it" does not say what counts as "it". Guessed: a `directory` source at this data dir, and a `user`-scope plugin; anything else named `factory` is an error, not an auto-remove (AD-4).
- `prd.md` §6 FR-4 Failure: silent on what the band and pane show after a failed poll. Slice-02 chose to clear them (AD-3); polish kept that, though the local review argued for keeping the last good reading.
- `qa-guide.md`, written from the §10 Human row, inherited "nothing shows" for the no-run check, which FR-4's pane text contradicts. Fixed in the guide; the PRD row is left as written.
- `/factory-board`'s toggle on a pane that is open but behind another tab: the types do not say whether `$.ui.open` on an open id raises it, so the toggle still closes it (two presses at worst).

## What I wished existed

- The Claude Code plugin types and `reference.md` on disk, written by `claude plugin validate` or a `claude plugin types` verb. Every check of the mod's API in polish went through a zstd frame cut out of the binary by an earlier slice, in `/tmp`, pointed at another lane.
- A way to drive a mod in a live session headlessly, so `/ux` had a harness for it.

## Project friction → `/gate audit`

None with evidence: the gate ran green at both checkpoints and after polish, and its `claude plugin` half is what proves the mod at all.

## Framework friction → factory

- `factory review collect` sent `docs/features/013-live-status/log.jsonl` and `state.toml` into pass 1, files only the binary writes; pass 1 still spent most of its time on docs. The pass planner could leave tool-written files out.
- `polish` § 7 (prune) after a single run's branch: every test was written by the slices of this run and already scoped tightly; the step found nothing. It is cheap, so keep it, but it is the step most likely to stay empty.
- `polish` § 1, UX row: "no harness → say so" is right, but a mod is user-visible with no harness road at all; the skill could name `claude plugin test` as the mod's harness so the skip is not a coverage hole by default.

A comment or name the leaks check should have caught: none found.

## Score

4 — the feature landed as the PRD drew it with no red gate, and the review stopped an install that would have reported success while loading a stale mod; one point off for the plugin API being reachable only by carving it out of the binary.
