# Slice 04 — factory review second

**Source**: [`prd.md`](prd.md) → §8 FR-6 (the command, its config, the probe, the passes, the partial text, the skill edits), and §3 FR-1's `models.md` decision and its last acceptance bullet. **Depends on**: 01 (the skill text it edits lives in `skills/` only after the port, and `models.md` is at `skills/_shared/models.md`). Independent of 02 and 03: nothing here reads a manifest.
**Created**: 2026-10-13

**Progress**:

- **Built.** `[review]` in `src/config.rs` (`agent`, `model`, `timeout_secs`, `pass_lines`, embedded defaults, `TOP_KEYS` gains `review` beside slice 05's `source`). `src/review.rs`: the probe, `group` (passes in numstat order, an oversized file alone and flagged), `parse_numstat` (`-z --no-renames`, a binary counts 0), `run_logged` (stdout and stderr to files, `try_wait` every 50 ms, SIGKILL to the reviewer's process group at the timeout), `parse_stream` (`result` wins, else the joined `assistant` text), the three prompts. `src/cli/review.rs` + `factory review second` in `src/main.rs`: every flag optional to clap, the combination checked in code (exit 1 with a usage line). The stub `tests/stubs/cursor-agent` (node, `100755`, argv logged as one JSON array per line). `tests/review.rs`: 8 tests covering every acceptance bullet of task 4 and the stub's own test. Skill text: `polish/SKILL.md` §1 keeps `:52` and the fallback, the six invocation paragraphs become two (the command, its exit codes); `prd/SKILL.md:99` names the `--doc` call and the exit-2 fallback; `models.md` § Second opinion keeps the rationale, "unconditionally" without the path, one line on the command and `[review]`, and the fallback. `tests/skills_bundle.rs` asserts the forbidden strings are gone.
- **Inner loop:** 3 runs, 2 red, about 2 min of runs. Red 1: `cargo fmt --check` on the new files. Red 2: `"\0010"` in a numstat test literal is an octal-looking escape (a compile error); the records are now joined with `"\0"`. Green: 458 tests, 65 s.
- **Deviations:** AD-3 (renames, empty diff → exit 2, process-group kill, probe log), AD-4 (fixtures at 20 s except the timeout test), AD-5 (`src/config.rs` carries slice 05's `[source]` hunks, which its commit left in the tree).
- **A misspelled flag** (`--dif main`) still exits 2 through clap before any code runs. The skill then starts its fallback, which is the safe side.
- **Prove (task 7)** is left for the checkpoint: the stub transcript, the live run scoped to `src/review.rs src/cli/review.rs`, and the empty `models.md` grep.

**Slice**: After this slice the cross-vendor second opinion is one command. `factory review second --diff <base> [-- <paths>]` probes `cursor-agent --list-models`, splits the diff into passes of at most 600 changed lines, runs each pass read-only under a timeout with the stream logged under the data dir, and prints one markdown section per pass. `--doc <file> --prompt "<text>"` runs one pass over a document. A dead auth costs nothing and says `unreviewed: <reason>` with exit 2; a timeout keeps the text it had. `/polish` and `/prd` call the command, and `models.md` loses its invocation block. A stub `cursor-agent` beside the stub `claude` proves it all inside `cargo test`.

**Highest risk: the live shape of `--stream-partial-output` was never recorded.** `tests/stubs/cursor-review.jsonl` was captured without it: three whole `assistant` events and a `result` whose text equals their concatenation. Live deltas might arrive as many small `assistant` events and then a full one repeating them, which would print the partial text twice. The `result` text is authoritative whenever it exists, so only the timeout path is exposed. The owner's FR-6 human row is the proof; do not add delta handling for a shape nobody has seen.

## Current code / environment

- **The recorded stream**: `tests/stubs/cursor-review.jsonl`, 16 lines, committed with the slicing commit. Line 1 is `{"type":"system","subtype":"init","model":"Grok 4.7 256K High",…}`, a display name and not the id; lines 7, 14 and 15 are `{"type":"assistant","message":{"content":[{"type":"text","text":…}]}}`; line 16 is `{"type":"result","subtype":"success","result":…}`, equal to the three texts joined. Lines 3-6 are `thinking` deltas and 8-13 are `tool_call` events. It is a real review of this repo's PRD-003.
- **Expired auth today**: `cursor-agent --list-models` prints `Error: Authentication required. Run 'agent login'…` and exits 1 (2026-10-13).
- **The child-process pattern**: `src/worker.rs:286-297` sends a worker's stdout to a file (`Stdio::from(stream_file)`) and stderr to another, then polls `try_wait` (`:361-366`) and kills with `kill()` (`:354-359`). A child whose stdout is a pipe nobody drains blocks once the pipe fills; the stream is about 34 KB per pass, so use files.
- **Config**: `src/config.rs:20-24` `Config { budget_weighted_tokens, workers }`, `#[serde(default)]`; `load(config_dir)` (`:113-136`) reads `<config_dir>/config.toml` and warns on top-level keys outside `TOP_KEYS` (`:103`, `["budget_weighted_tokens", "workers"]`). `paths::config_dir()` honours `FACTORY_CONFIG_DIR` (`src/paths.rs:76-78`), `paths::data_dir()` honours `FACTORY_DATA_DIR` (`:20-26`). The harness sets both per fixture (`tests/harness/mod.rs:517-563`) and puts `tests/stubs` first on `PATH`.
- **The stub pattern**: `tests/stubs/gh:1-37` is a node script that appends its argv to `$FACTORY_STUB_DIR/gh.log` and answers from scenario files. `tests/stubs/claude` is the larger version of the same.
- **Exit codes**: clap prints and exits 2 on a parse error before any code runs (`src/main.rs:53` `Cli::parse()`), and every other verb's usage error is 2 (`tests/docs_v3.rs:181`, `tests/docs_ideas.rs:115`). Review's 2 means "unreviewed, start the fallback".
- **The skill text this slice replaces**, as ported by slice 01: `skills/polish/SKILL.md:52` (the cross-vendor rationale, kept), `:54` (scope it, about 600 lines a pass), `:56` (check `--list-models` first), `:58` (tell it what the others found), `:60` (run the "A diff" invocation in `models.md`), `:62` (empty or killed is unreviewed), `:64` (the fallback). `skills/prd/SKILL.md:99` names "the `second opinion` role … Cursor CLI, cross-vendor and unconditional; a fresh agent on the judgment model only if that run fails". `skills/_shared/models.md:24-69` is `## Second opinion, specifically`: the Cursor path (`:28`), the default model and rate-limit history (`:32`), the three flags (`:36-40`), two ```` ```bash ```` blocks (`:44-50`, `:56-62`), the fallback (`:68`). The roles table (`:7-18`) names only Claude tiers.
- **The old prompt**, `models.md:49`: "Adversarial code review. The diff of <paths> against <base> is in <file>: read it, and read any file in the repository you need for context. Report only defects you can point to at file:line with a failure scenario; no praise, no style. Do not edit anything." A diff file outside the workspace worked in every run that used it (`:52`).
- **Deliberate non-changes** — *(prd §14 "Not handled: a `reviews` row in the metrics store. … no retro asked for the number." Do not record reviews in the metrics database.)*

## Tasks

### 1. `[review]` in the user config — high — blocked by: —
**Source**: §8 FR-6: "Config, `[review]` in `~/.config/factory/config.toml`: `agent = "cursor-agent"` (name or path), `model = "grok-4.7-high"`, `timeout_secs = 900`, `pass_lines = 600`. Defaults are embedded, so a missing file or key works".

`src/config.rs`: `pub review: Review` on `Config`, `#[serde(default)]`, with those four fields and those defaults. `TOP_KEYS` gains `"review"`. Slice 05 adds `"source"` beside it; the two lines merge by hand.

**Acceptance:** unit tests in `src/config.rs`: no file → the four defaults; a file with only `[review] timeout_secs = 2` → `2` and the other three defaults; a `[review]` table produces no unknown-key warning.

### 2. The stub `cursor-agent` — high — blocked by: —
**Source**: §8 FR-6 Acceptance: "A stub `cursor-agent` in `tests/stubs/` (beside the stub `claude` and `gh`) that replays a recorded stream". §12 Seed state: "a fixture diff and a recorded Cursor stream for FR-6".

`tests/stubs/cursor-agent`, node, executable, in the `gh` stub's pattern. It appends one JSON line of argv to `$FACTORY_STUB_DIR/cursor-agent.log`. It reads `$FACTORY_STUB_DIR/cursor-agent.json` when present:

- `list_models`: `"ok"` (default) prints a model list holding `grok-4.7-high` and exits 0; `"auth"` prints the `Authentication required` line and exits 1; `"no-model"` prints a list without it and exits 0.
- `sleep_after_assistant: n`: replay up to and including the n-th `assistant` event, then sleep 60 s.

Without either, a review call replays `cursor-review.jsonl` line by line to stdout and exits 0.

The trap: the harness finds the stub by name on `PATH`, so it must be committed executable. Write it, `chmod +x` it, and check `git ls-files -s tests/stubs/cursor-agent` shows `100755` after the commit.

**Acceptance:** the stub's own test in `tests/review.rs`: `cursor-agent --list-models` exits 0 by default and 1 under `"auth"`; a review call's stdout equals the recorded file byte for byte.

### 3. The probe, the passes and the parse — high — blocked by: 1
**Source**: §8 FR-6: "Probe first, spend nothing on a dead auth. `<agent> --list-models`: non-zero exit, a login prompt, or the configured model absent from the list → stdout `unreviewed: <reason>`, exit 2, no pass started." "Diff mode splits into passes. `git diff <base>...HEAD --numstat -- <paths>` sizes each file; files are grouped in order into passes of at most `pass_lines` changed lines, a file never split, a file larger than the cap alone in its pass and said so. Each pass writes `review-<n>.diff` and runs one invocation with the four load-bearing flags (`--trust`, `--mode ask`, `--output-format stream-json`, `--stream-partial-output`) under the timeout, logging to `<data dir>/reviews/<timestamp>/review-<n>.jsonl`". "A timeout keeps what it found. The stream's assistant text is accumulated as it arrives; on a kill the partial text is printed under a `(timed out after N s, partial)` header".

`src/review.rs` (new, in `src/lib.rs`):

- **Probe.** `<agent> --list-models` with output to a file, under `timeout_secs`. Non-zero exit, output containing `login` or `Authentication required`, or no whitespace-separated token equal to `model` → `unreviewed: <reason>`, where the reason is the exit code, the first output line, or `model <m> not in --list-models`.
- **Passes.** Lines per file = added + deleted from `--numstat`; a binary file's `-` counts 0. Walk the files in numstat order, closing a pass when the next file would push it past `pass_lines`. A file over the cap is closed alone and flagged `(over <cap>, alone)`.
- **One pass.** Write `git diff <base>...HEAD -- <the pass's files>` to `review-<n>.diff` in a fresh `<data_dir>/reviews/<YYYYMMDD-HHMMSS>/` (suffix `-2` when it exists). Spawn `<agent> -p --trust --mode ask --output-format stream-json --stream-partial-output --model <m> "<prompt>"` in the repo root, stdout to `review-<n>.jsonl`, stderr to `review-<n>.err`. Poll `try_wait` until `timeout_secs`, then kill.
- **Parse.** Read the `.jsonl` line by line, skipping lines that are not JSON. A `result` event's `result` is the pass's text. With no `result`, the pass's text is the `text` items of every `assistant` event's `message.content`, joined, and a killed pass prints it under `(timed out after N s, partial)`. No text at all → `unreviewed`.
- **Prompt.** The old one from `models.md:49`, with the pass's files, `<base>`, and the absolute diff path.

The trap: a pipe for stdout deadlocks a pass that writes more than the pipe holds while nobody reads it, and the timeout then fires on a working review. Files, as `src/worker.rs:296-297` does.

**Acceptance:** unit tests in `src/review.rs`: grouping `[300, 250, 700, 150, 100]` at 600 gives `[[a,b],[c],[d,e]]` with `c` flagged; a single 601-line file is alone and flagged; parsing the recorded stream returns line 16's `result`; parsing its first 14 lines (no `result`) returns the texts of lines 7 and 14 joined.

### 4. The command, its modes and its output — high — blocked by: 2, 3
**Source**: §8 FR-6: "Two modes. `factory review second --diff <base> [-- <paths>]` for `/polish`; `factory review second --doc <file> --prompt "<text>"` for `/prd`." "`--others <file>` passes the other reviewers' one-line summaries with the question "what would fixing those still miss"". "Output is markdown on stdout for the skill to triage: one `## Pass n — <files> (<lines> lines, <seconds> s, <model>)` per pass, the findings text under it, `unreviewed` for an empty pass. Exit 0 when at least one pass returned text, 2 when none did, 1 on a usage error. The log path is printed last."

`src/main.rs`: `/// Ask a reviewer from another vendor` `Review { #[command(subcommand)] verb: ReviewVerb }` with `Second(SecondArgs)`: `--diff <base>`, `--doc <file>`, `--prompt <text>`, `--others <file>`, and trailing paths after `--`. `src/cli/review.rs` (new). `tests/cli.rs:12-14`: add `"review"`.

- Doc mode is one pass: no `git diff`, the prompt is `<text>`, then the document's absolute path with "read it, and read any file in the repository you need for context. Do not edit anything." The header names the document and its line count.
- `--others`: the file's text and "Besides your own review: what would fixing those still miss?" go at the end of every pass's prompt.
- The header's `<model>` is the configured id, not the stream's `system/init` display name.
- The last line is `log: <the reviews dir>`.

The trap: clap's own usage errors exit 2 (`src/main.rs:53`), which the skills read as "unreviewed". Keep every flag optional in clap and check the combination in `src/cli/review.rs`, so both modes at once, neither, `--doc` without `--prompt`, or `--prompt` without `--doc` exit 1 with a usage line. A misspelled flag still exits 2 through clap; note it in Progress, since the skill then runs its fallback, which is the safe side.

**Acceptance:** `tests/review.rs` on a harness fixture with the stub first on `PATH` and `[review] timeout_secs = 2` in the fixture's config dir. The fixture branch adds files `a` to `e` of 300, 250, 700, 150 and 100 lines over `main`.

- `--diff main`: exit 0; three `## Pass` headers, the second naming `c` alone with `(over 600, alone)`; three review calls in `cursor-agent.log`, each argv holding all four flags and `--model grok-4.7-high`; three `review-<n>.diff` and three `.jsonl` files under the printed log dir.
- `--diff main -- a b`: one pass.
- `list_models: "auth"`: stdout `unreviewed: …`, exit 2, and the log holds only the `--list-models` call. Same for `"no-model"`.
- `sleep_after_assistant: 2` with `-- a b` (one pass): exit 0 within about 3 s; stdout holds `(timed out after 2 s, partial)` and the texts of the first two `assistant` events.
- `--doc docs/features/001-fixture/prd.md --prompt "find what cannot be true"`: one pass; the logged argv holds the prompt text and the document's path.
- `--others notes.txt`: every logged prompt holds the file's text.
- `--doc x` alone, and `--diff main --doc x --prompt y`: exit 1, no call logged.

### 5. The skills call the command; `models.md` loses the invocation — high — blocked by: 4
**Source**: §8 FR-6: "The skills call it. `polish/SKILL.md` §1 and `prd/SKILL.md` §4 replace the invocation paragraphs with the command, its exit codes and the fallback; `models.md` loses the invocation block (FR-1)." §3 FR-1: "`models.md` shrinks to the roles. The `## Second opinion, specifically` invocation block, the Cursor path, the default model and the rate-limit note move into FR-6 and `config.toml`". FR-1 Acceptance: "`models.md` contains no shell invocation, no binary path and no third-party model name (`grok-…`, `gpt-…`). The Claude tier words in the roles table … stay".

- `skills/polish/SKILL.md`: keep `:52`. Replace `:54-64` with three short paragraphs. The command: `factory review second --diff <base-branch>`, scoped with `-- <paths>` when the diff mixes risky and trivial files, and `--others <file>` holding the local and security reviewers' one-line findings when they found high-severity defects. The exit codes: 0 is findings to verify in step 2; 2 is unreviewed, so the fallback starts and is said out loud; 1 is a usage error to fix. The fallback, as `:64` says it. Keep the facts the old paragraphs taught as one line each where the command does not already enforce them: an empty or killed pass is unreviewed, never "no findings".
- `skills/prd/SKILL.md:99`: the reviewer is `factory review second --doc docs/features/{NNN}-{slug}/prd.md --prompt "<the instruction in the next paragraph>"`; exit 2 → a fresh agent on the judgment model, never a fork.
- `skills/_shared/models.md`: `## Second opinion, specifically` keeps the cross-vendor rationale (`:26`), "unconditionally" and why presence is no signal (`:28`, without the path and the rename story), and one line naming `factory review second` with its two modes and that `[review]` in `~/.config/factory/config.toml` holds the agent and model. `:30-69` go, except the fallback sentence (`:68`).

The trap: slice 07 hashes `skills/` into `skills/bundle.lock`. These edits land before it (07 depends on 04), so they cost no bump here. A skill edit after 07 does.

**Acceptance:** a test in `tests/skills_bundle.rs` reads `skills/_shared/models.md` and asserts it holds none of ```` ```bash ````, `grok-`, `gpt-`, `gemini-`, `~/.local/bin`, `--trust`, `cursor-agent -p`; `skills/polish/SKILL.md` and `skills/prd/SKILL.md` each contain `factory review second`; `grep -rn "A diff\" invocation" skills/` is empty.

### 6. Verify — medium — blocked by: 1-5
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 7. Prove — at the checkpoint — blocked by: 6
Rows from [`prd.md`](prd.md) §12 this slice proves:

- **FR-6, stub row**: run `factory review second --diff` against the stub: three passes, the oversized file alone; `--list-models` failing → exit 2; a timeout keeps the partial text; `--doc` records the prompt. Keep the `tests/review.rs` transcript.
- **FR-6, live row**: the checkpoint runs the real `cursor-agent` once on this branch. Clarified at slicing, 2026-10-13 (written into the row): scope it to this slice's source, `factory review second --diff main -- src/review.rs src/cli/review.rs`, so one pass stays under the cap. It prints findings and the log path, or `unreviewed: <reason>` with exit 2 when the auth is gone. Both are a pass; a hang past `timeout_secs` or a missing log path is a fail.
- **FR-1, `models.md` bullet**: `grep -nE 'grok-|gpt-|gemini-|local/bin|```bash' skills/_shared/models.md` prints nothing.

**Acceptance:** after the checkpoint, Progress holds the test transcript, the live run's stdout and exit code, and the empty grep.
