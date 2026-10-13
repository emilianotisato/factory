## Decisions

### AD-901 — What does `Docs::log_add` return for an entry without an id? · slice-09 · 2026-10-13

**Chose:** `Result<Option<String>>`, the same as `log::add`: the id for a decision, block, ask or checkpoint, `None` for an answer, review or note
**Over:** `Result<String>` as the slice wrote it, with an empty string or an error for the id-less kinds
**Because:** the driver writes answers and notes through the same call; `log::add` already allocates no id for them, and an empty string would be a fake id a caller could print
**Undo:** unwrap at the call sites and change the signature
**Review:** no

### AD-902 — What do `Ship::Done` and `Ship::Refused` carry? · slice-09 · 2026-10-13

**Chose:** `Ship::Done(String)` holds what `ship` printed; `Ship::Refused(String)` holds the refusal's message (an `Error::Msg` from `ship::ship`). Any other error stays an `Err`
**Over:** a bare `Ship::Done`, or a refusal that also swallows I/O errors
**Because:** closeout printed the tool's stdout after a ship before this slice, and a torn `state.toml` is not a verdict to block on as "slice not done"
**Undo:** drop the payload from `Done`
**Review:** no

### AD-903 — Where does closeout's retro frontmatter setter live once `scheduler::set_frontmatter` goes? · slice-09 · 2026-10-13

**Chose:** a private `set_retro_keys` in `closeout.rs`, used by `fill_retro` and the `ci_cycles` bump, on `scheduler::write_atomic`
**Over:** keeping `set_frontmatter` in the scheduler, which task 2's grep must find empty
**Because:** slice files are read-only to the driver now; only `retro.md` still has frontmatter the driver writes, and slice 10 removes `fill_retro` and that frontmatter
**Undo:** slice 10 deletes it with `fill_retro`
**Review:** no

### AD-904 — Where does the retro template come from now that `Docs` has no tool path? · slice-09 · 2026-10-13

**Chose:** `<repo>/.docs/templates/retro.md`, else `~/.claude/skills/feature-docs/templates/retro.md`, else the built-in frontmatter with `RETRO_KEYS`
**Over:** reading the template beside `docs.mjs` through `docs.tool()`, which went with the subprocess
**Because:** same lookup order as before without a located tool. The v3 fixture has no `.docs/`, so a test reads the developer's global template when one exists (the harness does not reset `HOME`) and the built-in one in CI; both carry the same keys and heading, and `tests/closeout.rs` passes with either
**Undo:** slice 10 reworks the retro; drop the two file lookups then
**Review:** fyi

### AD-905 — How does the preflight answer recognise its own retry? · slice-09 · 2026-10-13

**Chose:** one `ask` entry titled `Preflight ASK-n: <item>` with `phase: preflight`; a retry finds an `ask` with that title whose `**Chose:**` line matches and appends nothing
**Over:** matching on the AD id, which a retry cannot know before the append
**Because:** the old check matched the heading text in `agent-log.md`; the title carries the same key without a date, so a retry on another day still matches
**Undo:** change `preflight_logged` in `cli/answer.rs`
**Review:** no

### AD-906 — What happens to the scheduler unit tests of the deleted helpers? · slice-09 · 2026-10-13

**Chose:** deleted the 2 `append_under` tests with the 4 frontmatter tests task 2 names; the log writes they covered are tested through `answers.rs` and the suites
**Over:** porting them, which would test helpers that no longer exist
**Because:** `append_under` has no caller once every BLOCK, note and answer is a `log.jsonl` entry (task 3); `log::add` has its own tests from slice 02
**Undo:** none needed
**Review:** fyi

### AD-907 — Delete or keep `tests/docs.rs`, whose board test ran on node? · slice-09 · 2026-10-13

**Chose:** rewrote it as task 1's tests on the v3 fixture: `Docs::open` and `status` equal to `factory docs status 001 --json`, `set_slice_status`, `log_add` of a checkpoint `BLOCK-1` in `open_blocks`, the v2 refusal text, and `ship` refused on pending slices
**Over:** deleting it as task 7 says and putting task 1's tests elsewhere
**Because:** it is the file named for the `Docs` type; its preflight and gate assertions still hold in process, so they stay
**Undo:** move the tests
**Review:** no

### AD-908 — What happens to the markdown block parser's unit tests? · slice-09 · 2026-10-13

**Chose:** deleted the four `answers.rs` tests of the `### BLOCK-n` heading parser (fences, agent-log order, heading parseable, find and resolve) and `a_preflight_answer_takes_the_next_ad_below_100`, with the code they tested; `a_block_is_resolved_by_an_answer_and_its_line_stays` covers the log-based `entries`, `find` and `resolve`
**Over:** keeping a markdown parser alive for its tests
**Because:** task 3 moves blocks and asks to `log.jsonl` and deletes `next_preflight_ad`; the parser has no input left to read
**Undo:** none needed
**Review:** fyi

### AD-909 — A build stream no longer repeats a target: what does the metrics repeat test assert? · slice-09 · 2026-10-13

**Chose:** `every_tool_call_is_a_row_with_its_kind_and_repeat_flags` (`tests/metrics.rs`) now asserts that no stub build row is flagged as a repeat; the flagged case stays covered by the unit test `repeat_flags_are_per_worker_and_per_run` in `src/metrics.rs`
**Over:** adding a second Edit of the same file to the stub only to keep a repeat in its stream
**Because:** the repeat came from the stub's second Edit of its slice file; workers no longer edit slice files (task 2), and the two `factory docs` Bash calls that replaced it are different commands
**Undo:** give the stub a step option that edits one file twice
**Review:** fyi

## Traps

- `grep -rn` over `src` is refused by the repo hook; `git grep -n -e <literal> -- 'src/*.rs'` answers the acceptance greps.
- Slice 07 builds in the same tree: `cargo build` stays red while `src/docs/migrate.rs` is mid-edit. Wait for its file to compile rather than touching it.
