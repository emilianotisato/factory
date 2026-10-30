---
name: deliver
description: Drive an unattended `factory` run of an agreed PRD from one thin session — slice it if needed, start it, wait on it, relay its questions to the owner, then run the review interview.
---

# /deliver NNN — the cockpit

You are the cockpit of one `factory` run, not its builder. Never read the board, the slice files or the log to decide anything about the run: `factory wait` prints every event the owner must see, and `factory status <id>` is the one look at the board (ten lines). Keep replies short; the owner may be reading on a phone.

## 1. Start

0. `factory docs status NNN`. On the default branch → `git switch` to the `branch` it names (`-c` when it does not exist yet), so nothing below commits on the default branch. Then `factory docs doctor`, its `skills` line per [_shared/stale-tooling.md](../_shared/stale-tooling.md). `not sliced` → invoke `/slice NNN from /deliver` with the Skill tool, then commit what it wrote by pathspec, `docs(NNN): slice PRD-NNN`.
1. `factory run NNN` (add `--budget <weighted tokens>` or `--hypothesis "<one line>"` only when the owner gave one). It prints the run id; tell the owner the id — it is the handle for `factory wait <id>`, `factory status <id>` and `factory stop <id>`.
2. If it reports a live or resumed run, that run id is the one to drive. So the owner may `/clear` and type `/deliver NNN` at any time: this line picks the run up.
3. Exit 1 is a preflight refusal: read each `✗` line, fix the ones below, then `factory run NNN` again.
   - `working tree not clean` with every dirty path under `docs/features/NNN-*` → `git add` them and commit by pathspec, `docs(NNN): …`. Any other dirty path → ask.
   - `docs check` errors in this feature's own files → fix them and commit.
   - `not sliced — /slice NNN` → slice as in step 0.
   - Every other `✗` line → one `AskUserQuestion` each, its fix as option 1; do what the answer says before the retry. Never run `/gate` or `/lanes` unasked.

## 2. Loop until the run ends

Run `factory wait <id> --timeout 3300` in a background Bash whose own `timeout` is above 3,300,000 ms, and do nothing else until it returns. (A 55-minute poll stays inside the 1-hour prompt cache; `wait` returns early on every event the owner must see.) On return:

- exit 3 (`still running: …`): wait again. Say nothing unless the owner asks.
- exit 4 (`driver dead; run: factory run NNN`): run `factory run NNN` (if it says the run is live, a driver already took it), then wait again.
- exit 0: one line per event, in order. For each line, `PushNotification` the line (under 200 characters), then:
  - `BLOCK-n …` or `ASK-n …`: the owner decides. Run `factory status <id>` once for context, then `AskUserQuestion` with the question's text, the Recommended option first, the alternatives after it, and `Other` for a free answer. Then `factory answer BLOCK-n "<the choice>"` (or `ASK-n`) and wait again. `wait` prints a question again on every call until it is answered, so if the owner will not answer now, say that `factory wait <id>` brings it back, and end.
  - `paused until …`: information; the run resumes by itself. Wait again.
  - `stand-in: …`: information; the polish worker left the retro or the PR body unwritten and factory filled it from a template. The PR body's first line says so. Wait again, and name the file when §3 gives the owner the PR URL.
  - `subagents unread: …`: information; a worker's subagent transcripts could not be read, so its subagent tokens stay blank in `report`. The run goes on. Wait again.
  - `parked: …`: a cap was reached and the driver has exited. Ask the owner for a new budget; on an answer run `factory run NNN --budget <n>` and wait again. With none, report that `factory run NNN --budget <n>` resumes it, and end: `wait` prints the park again on every call while it stands.
  - `stopped: …`: the owner stopped it. Report; `factory run NNN` resumes it later. End.
  - `failed: …`: report the reason with `factory status <id>`. End.
  - `finished: <pr url>`: go to §3.

If the owner asks to stop: `factory stop <id>` — it says how many workers are still in flight; the run ends when they exit.

## 3. Review interview

Read the log now, the only time this skill reads it: `factory docs log show NNN --decide` for the `decide` ADs no review answers yet, then `factory docs log show NNN --json` for the `fyi` decisions with no `review` entry pointing at them (`--decide` drops those); skip the ones with no flag. None → say so and ask nothing.

1. `AskUserQuestion` once: "Review all n now (Recommended) / Only the d decide ones / Later". Later → stop here; the PR stays as it is.
2. Rounds of at most four questions per `AskUserQuestion` call. Question = the AD's own question with `AD-{n} · slice-{NN}`; Header `AD-{n}`; options = what it chose and its `Over:` alternative (a third only when a real one exists), each description carrying its cost (`Undo:` for the one not chosen). The recommendation goes first, re-judged with what the run learned since; `Other` is free text.
3. Apply: one `factory docs log add NNN --kind review --ref AD-{n} --verdict kept --title "AD-{n} kept"` per kept answer, and `--verdict changed --title "AD-{n} changed" --body-file -` with the "to what" on stdin per changed one; never edit the log by hand. A changed AD gets its code change by a subagent on the feature branch: inner loop, then the gate once, one commit by pathspec. Append `**Owner review ({date}):** k kept, c changed (AD numbers)` to the feature's `retro.md`. Push once.
4. Tell the owner the PR URL, that it is a draft, and that merging is theirs.

## 4. A dead cockpit, a compaction, a lost context

Start here with the run id (gone → `factory status` lists the live runs): `factory wait <id>` replays every event the old session missed and repeats every question and park still open, shown or not (`--replay` repeats them all), then continue at §2.
