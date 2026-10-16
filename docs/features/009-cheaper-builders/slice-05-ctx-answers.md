# Slice 05 — ctx answers: def trims and the denial's answer

**Source**: [`prd.md`](prd.md) → §3 FR-5, FR-6. **Depends on**: 01 (the answer goes on the deny that slice 01 rewrites: the same arms of `bash()` in `src/cli/hook.rs`, and the same `assets/lookup.md`. Built after it, this diff is about the answer only)
**Created**: 2026-10-15

**Progress**:
- Built: `print::defs()` decides once whether a body prints (exactly one match) and uses it for the loop and the cut count; several matches print locations, signatures and one line naming `--kind` and `--in` (AD-4). `def` gained `--in`; `def` and `refs` take a file or a directory (`value_name = "PATH"`), `unreadable()` asks for metadata. `def::find()` takes a root. A grep or rg the hook denies carries `ctx refs` and `ctx def --no-body` output under the hint when either found a hit, decided on `refs::Found.total` and `def::Found.hits` (AD-5); the defaults come from `refs::MAX` and `refs::PER_FILE`, shared with the CLI. `def` help and the pack's `# Lookup` say a body comes when one definition matches.
- Tests: `def_prints_a_body_only_when_one_definition_matches`, `refs_in_a_file_lists_only_that_file`, `def_help_says_when_a_body_comes` (tests/ctx.rs), `a_denied_grep_carries_what_ctx_finds` (src/cli/hook.rs), the `# Lookup` assertion in tests/loop.rs. Snapshots pass unblessed; the exact-hint tests pass unchanged.
- Inner loop: 1 run, 0 red, 2 min. A live denial on this repo (debug build) took 0.3 s.
- Deviation: `src/pack.rs` changed one call site for `def::find()`'s new root argument.
- Prove: FR-5 and FR-6 rows left for the checkpoint.
- FR-5 (checkpoint 2): `factory hook` fed `grep -rn alpha src` in a temp-dir fixture denies with the hint, then `ctx refs alpha` and `ctx def alpha --no-body` output; a grep with no hit gets the hint alone — evidence: cp-2/fr5-hook-answer.txt
- FR-6 (checkpoint 2): `ctx def notify` on the ctx fixture prints two locations and the narrowing line, `--in php/OtherService.php` prints the one body, `refs planted --in refs/caller02.php` lists that file only — evidence: cp-2/fr6-ctx-def-refs.txt

**Slice**: `ctx def` prints a body only when exactly one definition matches; with several it prints each location and signature, then one line naming `--kind` and `--in` to narrow. `--in` takes a file or a directory, for `def` and `refs`. A grep the hook denies carries ctx's answer under the hint: `refs <word>` and `def <word> --no-body`, run in-process from the worker's cwd. The worker gets in one turn what it would have fetched in the next.

## Current code / environment

- **`def` prints every body**: `ctx::run()` (`src/ctx/mod.rs:86`) calls `print::defs(&a.symbol, &found, !a.no_body)` (`:94`), and `print::defs()` (`src/ctx/print.rs:8`) prints the body of each hit (`:10-17`). Its trailer counts the cut bodies over every hit when `body` is set (`:46-50`).
- **`def` has no `--in` and walks the whole base**: `DefArgs` (`src/ctx/mod.rs:31-44`) has `symbol`, `--kind`, `--no-body`, `--vendor`. `def::find(base, name, want, opts)` (`src/ctx/def.rs:129`) walks `base`. `refs::find(base, root, name, caps, opts)` (`src/ctx/refs.rs:46`) already takes a root and prints paths relative to `base`.
- **`refs --in` takes a directory only**: `RefsArgs` `--in`, `value_name = "DIR"`, doc "Search this directory only" (`src/ctx/mod.rs:50-52`). The walker takes a file root as it is (`ignore::WalkBuilder`, `src/ctx/walk.rs:48-49`), but the check before it does not: `unreadable()` (`src/ctx/mod.rs:126-135`) calls `read_dir`, so `--in <file>` exits 2 today.
- **The texts that promise a body**: the `def` help "Where a symbol is defined, with its body" (`src/ctx/mod.rs:23`) and the pack's `# Lookup`, "where a name is defined, with its body" (`assets/lookup.md:3`). Slice 01 rewrites `:7` and `:9` of the same file.
- **The ctx fixture defines no name twice**: every `def` snapshot ends `1 definition (searched 48 files)` or a miss (`tests/snapshots/ctx/def-*.txt`), so FR-6 changes no snapshot. `each_command_prints_its_snapshot_byte_for_byte_twice` (`tests/ctx.rs:84`) must pass without `FACTORY_BLESS=1`. A file added to `tests/fixture/ctx` changes "searched 48 files" in every snapshot, so the twice-defined case goes in a temp dir through `ctx_in(dir, args)` (`tests/ctx.rs:15`), as `every_vue_script_block_is_read` (`:449`) does.
- **A missing root still exits 2**: `a_bad_flag_or_an_unreadable_root_exits_2` (`tests/ctx.rs:566`) pins `refs planted --in nowhere` at exit 2 with "factory ctx: cannot read nowhere: ".
- **The denial is the hint alone**: `bash()` (`src/cli/hook.rs:121`) denies a grep or rg with `Verdict::Deny(grep_hint())` (`:142-145`). `here` is the cwd after the `cd`s (`:127-136`). The header promises "no I/O beyond stdin, the pack's first line and a `stat`" (`:1`).
- **A miss still prints a line**: "no definition of X found" (`src/ctx/print.rs:20`) and "no references to X found" (`:65`). The counts are `def::Found.hits` and `refs::Found.total`.
- **Tests that pin the hint alone stay exact**: `recursive_grep_over_the_repo_is_denied_with_the_verb` (`src/cli/hook.rs:881-915`) compares `Verdict::Deny(grep_hint())`, and every file of its fixture `repo()` (`:831-848`) holds `x\n`, so ctx finds nothing there. `tests/cli.rs:100-107` pins the hint's exact text in a fixture whose `src/` is empty.
- **The hook's budget**: `"timeout": 5` in the worker's settings (`src/worker.rs:98`). Past it, Claude Code runs the grep. *§3 FR-5 Accepted cost: "About 40 ms per denial on the replayed repos."*
- **Deliberate non-change**: *§3 FR-5 Rejected: "Rewriting the command through PreToolUse `updatedInput`: zero turns, but it changes a worker's command without telling it."* The hook denies and answers; it never rewrites the command.
- **Deliberate non-change**: *prd §7: "ctx index gaps and a report metric per question type stay in the ctx idea, items 5 and 6."*

## Tasks

### 1. `def` prints one body — high — blocked by: —
**Source**: §3 FR-6: "`def` prints a body only when exactly one definition matches. With several, it prints each one's location and signature, then one line naming `--kind` and `--in` to narrow."

`src/ctx/print.rs` `defs()`: print bodies when `body` is set and exactly one definition matched. With several, print each location and signature, and one line naming `--kind` and `--in` before the trailer. `--no-body` keeps its meaning for a single match.

**The trap:** the trailer counts cut bodies over every hit (`src/ctx/print.rs:46-50`). Decide "bodies printed" once and use it for both the loop and the trailer, or a withheld body is reported as cut.

**Acceptance:** a ctx test in a temp dir holding a name defined in two files: `def` prints two location-and-signature lines, no body lines, no "cut at" and a line naming `--kind` and `--in`. The snapshot test passes unblessed.

### 2. `--in` takes a file or a directory — high — blocked by: —
**Source**: §3 FR-6: "`--in` takes a file or a directory, for `def` and `refs`."

`src/ctx/mod.rs`: `DefArgs` gains `--in`, as `RefsArgs` has it; both take a file or a directory (`value_name`, doc). `def::find()` takes a root as `refs::find()` does and keeps printing paths relative to `base`. `unreadable()` asks for metadata instead of `read_dir`, so a file root passes and a missing one still exits 2.

**Acceptance:** ctx tests: with `--in` naming the file that holds one of the two definitions of task 1, `def` prints that one body and `1 definition`; `refs planted --in <file>` on the ctx fixture lists only that file's hits; `a_bad_flag_or_an_unreadable_root_exits_2` passes unchanged.

### 3. The texts say when a body comes — medium — blocked by: 1
**Source**: §3 FR-6: "The texts that promise a body say when one comes: the pack's `# Lookup` (`assets/lookup.md:3`) and the `def` help (`src/ctx/mod.rs:23`)."

`assets/lookup.md:3` and `src/ctx/mod.rs:23`: a body comes when one definition matches.

**Acceptance:** the slice-pack test in `tests/loop.rs` (`:405-414`) asserts the pack's `# Lookup` says the body comes when one definition matches; a ctx test finds the same in the output of `ctx def --help`.

### 4. The denial carries ctx's answer — high — blocked by: —
**Source**: §3 FR-5: "On an FR-4 denial, the hook runs ctx in-process from the worker's cwd: `refs <word>` with its default caps (10 hits, 3 per file …) and `def <word> --no-body`. It puts both outputs under the hint in the denial's reason. A ctx error, or no hit from either command, leaves the hint alone. The rule counts hits, never the text … The header line says the deny path reads the tree."

`src/cli/hook.rs`: on the grep and rg denies of `bash()`, call `refs::find()` and `def::find()` with `here` as the base and the pattern's identifier as the name, with `refs`' defaults (max 10, per file 3, no tests, no vendor), and print them with `print::refs()` and `print::defs()` without bodies. When either found a hit, the reason is the hint with both outputs under it. When neither did, or either returned an error, the reason is the hint alone. The header at `:1` says the deny path reads the tree.

**The trap:** the outputs are never empty, since a miss prints its own line (`src/ctx/print.rs:20`, `:65`). Decide on `def::Found.hits` and `refs::Found.total`, never on the printed text, or every denial carries two miss lines.

**Acceptance:** a hook test in a temp dir with `alpha` defined once and called twice under `src/`: `grep -rn alpha src` is denied, and the reason holds the hint, the definition's `file:line` and both call sites. `grep -rn nothing_here src` in the same dir is denied with the hint alone. `recursive_grep_over_the_repo_is_denied_with_the_verb` and `tests/cli.rs:100-107` still pass on exact equality.

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
`prd.md` § QA plan rows, driven below the UI:
- **FR-5**: run `factory hook` fed `grep -rn alpha src` in the temp-dir fixture; read the reason; keep the transcript.
- **FR-6**: run `factory ctx def` on a twice-defined name, with and without `--in <file>`, and `factory ctx refs planted --in <file>` on the ctx fixture; keep the transcript.

**Acceptance:** after the checkpoint, each row has its transcript in this slice's **Progress**.

**Code-review fixes (2026-10-15).**
- The denial's ctx answer walked from the cwd even when it lay outside the repo: `cd / && grep -rn main <repo>/src` walked the whole filesystem (39 s, a 1.15 MB reason in the security pass). A cwd outside the repo now searches the repo; tested in `a_denied_grep_carries_what_ctx_finds`.
- `--in` taking a file switched the root check from `read_dir` to `metadata`, so an unreadable directory searched nothing and exited 0. A directory is read again and a file opened, so it exits 2 as the help says; tested in `a_bad_flag_or_an_unreadable_root_exits_2`.
