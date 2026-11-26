# Slice 03 — Bash guard fixes, regression corpus and skill rules

**Source**: [`prd.md`](prd.md) → §6 FR-4, §7 FR-5, §9 QA plan. **Depends on**: 02 (edits `src/cli/hook.rs` after slice 02).
**Created**: 2026-11-26

**Progress**: cp-2/FR-1/transcript.txt · cp-2/FR-2/transcript.txt · cp-2/FR-3/transcript.txt · cp-2/FR-4/transcript.txt · cp-2/FR-5/transcript.txt

**Slice**: Fixes Bash `guard` false denials on `.push`/`->push`/`::push` method calls and non-backtick-exec interpreters (`python3`, `node`, `bun`, `deno`, etc.), widens `process_call` for missed Ruby/Perl/Lua/Bun/Deno/osascript/tclsh/PHP/Python process-spawning spellings, adds the `tests/fixture/hook/denied.json` regression corpus replayed in `tests/hook.rs`, adds the guard-loosening acceptance sentences to `skills/{prd,slice}/SKILL.md`, and bumps crate/plugin/bundle version from `0.16.0` to `0.17.0`.

## Current code / environment

- **`src/cli/hook.rs:194-205` (`mentions`)**: checks whole-word ASCII matches for `"gh"` and `"push"`. Because `.`, `>`, `:` are non-word bytes, `out.push(`, `arr.push(`, `$c->push(`, and `Vec::push` all return `true`.
- **`src/cli/hook.rs:511-616` (`interpreter`, `inline_script`, `script`, `process_call`)**:
  - `process_call(text)` does not receive `program`, so ``"`"``, `"%x"`, and `"qx"` trigger for EVERY interpreter (causing Python scripts with markdown backticks and JS/Node/Bun/Deno scripts with template literals `` `...` `` to register as process calls).
  - `CALLS` (`:592-610`) has 17 entries and misses `"__import__"`, `"Deno.Command"`, `"Bun.spawn"`, `"io.popen"`, `"do shell script"`, `"pcntl_exec"`, `"posix_spawn"`, `"posix_spawnp"`.
  - Lines `:612-615` check `["system(", "exec(", "spawn(", "popen("]` with `!text[..i].ends_with('.')`, which skips `os.system(`, `Kernel.system(`, `__import__("os").system(`, and `io.popen(`. Switching to `!prev_is_word_char` without adding `"pcntl_exec"`, `"posix_spawn"`, `"posix_spawnp"` to `CALLS` would regress those three!
- **`tests/hook.rs:45-241`**: six denied-case test functions hold 106 denied commands inline. `prd.md` frontmatter authorizes `tests/hook.rs` in `authorized_tests`.
- **Skill bundle lock (`Cargo.toml:3`, `mods/cockpit/.claude-plugin/plugin.json:3`, `skills/bundle.lock:2`, `tests/skills_bundle.rs:261`, `src/skills.rs:208-236`)**: `0.16.0` is already tagged on `origin`, so any edit to `skills/{prd,slice}/SKILL.md` fails `bundle_lock_matches` unless `Cargo.toml`, `Cargo.lock`, and `mods/cockpit/.claude-plugin/plugin.json` are bumped to `0.17.0` and `skills/bundle.lock` is regenerated via `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle`.
- **Deliberate non-changes (`prd.md` §11)**: do NOT add versioned interpreter names (`php8.3`, `ruby3.3`) to `interpreter()`, and do NOT put plan-id strings (`PRD-`, `FR-`, `BLOCK-`, `slice-`) inside `tests/fixture/hook/denied.json` (which is NOT in `docs/project.toml` `[leaks] allow`).

## Tasks

### 1. Scope `process_call` by interpreter, ignore method-call `.push`/`->push`/`::push` in `mentions`, and widen `process_call` — high — blocked by: —
**Source**: §6 FR-4: "Thread `program` from `interpreter()` -> `inline_script()` -> `script()` -> `process_call()` ... Keep `` ` `` ONLY for `ruby`, `perl`, and `php`; `"%x"` for `ruby` only; `"qx"` for `perl` only ... When matching `"push"` in `mentions()`, ignore occurrences immediately preceded by `.`, `->`, or `::` ... Add to `CALLS`: `"__import__"`, `"Deno.Command"`, `"Bun.spawn"`, `"io.popen"`, `"do shell script"`, `"pcntl_exec"`, `"posix_spawn"`, `"posix_spawnp"` ... Remove `!text[..i].ends_with('.')` skip for `"system("`, `"exec("`, `"spawn("`, `"popen("` (requiring prev byte not alphanumeric/`_`) ... For `ruby`, `perl`, `tclsh`, `wish`, match paren-less `"system "`, `"exec "`, `"spawn "` (and `\t`) at word start".

- `src/cli/hook.rs:194-205` (`mentions`):
  - Keep `"gh"` matching unchanged.
  - For `"push"`, after confirming whole-word boundaries (`!word(b[i-1])` and `!word(b[i+4])`), also ignore the match when `b[..i]` ends with `b"."`, `b"->"`, or `b"::"`.
  - Keep `mentions_are_whole_words` unit test green and add assertions that `out.push(line)`, `$c->push(1)`, and `Vec::push` return `false` while `git push`, `git-push`, `"push"`, `'push'`, and `$PUSH` still return `true`.
- `src/cli/hook.rs:511-616` (`interpreter`, `inline_script`, `script`, `process_call`):
  - Thread `program: &str` through `inline_script(program, words)` and `script(program, text)` into `process_call(program, text)`.
  - In `process_call(program, text)`:
    - Shared `CALLS` (22 entries — removing ``"`"``, `"%x"`, `"qx"` from the global list and adding `"__import__"`, `"Deno.Command"`, `"Bun.spawn"`, `"io.popen"`, `"do shell script"`, `"pcntl_exec"`, `"posix_spawn"`, `"posix_spawnp"`).
    - Interpreter-specific syntax:
      - ``"`"`` when `matches!(program, "ruby" | "perl" | "php")`
      - `"%x"` when `program == "ruby"`
      - `"qx"` when `program == "perl"`
    - Call-with-paren check: `["system(", "exec(", "spawn(", "popen("]` matched whenever `i == 0` or `!is_word(text.as_bytes()[i - 1])` (where `is_word(b)` is `b.is_ascii_alphanumeric() || b == b'_'`).
    - Paren-less calls for `matches!(program, "ruby" | "perl" | "tclsh" | "wish")`: match `"system"`, `"exec"`, `"spawn"` when preceded by start-of-string or non-word byte AND immediately followed by `' '` or `'\t'` (so `system "gh pr list"`, `exec "git", "push"`, `exec git push` all match).
- `tests/hook.rs`: add `guard_allows_push_method_calls_and_denies_widened_process_calls` testing:
  - Allowed: `python3 - <<'PY'` with markdown backticks and `out.push(line)`, `node -e "const a = []; a.push(1); console.log(\`${a}\`)"`, quoted heredoc writing an entry with backticks and `push`, `php -r '$c->push(1);'` with no backtick exec.
  - Denied: all 11 newly verified bypass spellings (`ruby -e 'system "gh pr list"'`, `perl -e 'exec "git", "push"'`, `python3 -c '__import__("os").system("gh pr list")'`, `bun -e 'Bun.spawn(["git","push"])'`, `lua -e 'io.popen("git push")'`, `ruby -e 'Kernel.system("gh")'`, `deno eval 'new Deno.Command("gh",{args:["pr","view"]}).outputSync()'`, `osascript -e 'do shell script "git push"'`, `tclsh <<'EOF'\nexec git push\nEOF`, `php -r 'pcntl_exec("/usr/bin/gh",["pr","list"]);'`, `python3 -c 'import os; os.posix_spawn("/usr/bin/gh", ["gh", "pr", "list"], {})'`).

**Trap**: In `interpreter()` (`:511-565`), `tclsh` / `wish` have `inline = &[]` and read heredocs via `stdin` (`:560`), so `tclsh <<'EOF'\nexec git push\nEOF` flows through `script(program, doc)` with `program == "tclsh"` — make sure `program` is passed on BOTH the `inline_script` path AND the `stdin` path!

**Acceptance:** `guard_allows_push_method_calls_and_denies_widened_process_calls` passes and fails if backticks stay global, if `.push` triggers `mentions`, or if any of the 11 bypass spellings is dropped.

### 2. Add `tests/fixture/hook/denied.json` regression corpus and replay test — high — blocked by: 1
**Source**: §7 FR-5: "Store every command `main`'s `guard` denies (all 106 existing denied commands from `tests/hook.rs:45-204` plus the 11 new FR-4 commands) in `tests/fixture/hook/denied.json` with two arrays: `"denied"` and `"allowed_on_purpose"` ... Add a single corpus replay test `hook_guard_replays_the_denied_corpus_and_allowed_exceptions` in `tests/hook.rs`".

- Create `tests/fixture/hook/denied.json` with schema:
  ```json
  {
    "denied": [ "...every command from tests/hook.rs:45-204 + all 11 new FR-4 commands..." ],
    "allowed_on_purpose": [
      { "cmd": "...", "why": "..." }
    ]
  }
  ```
  Include all 106 existing denied commands from `tests/hook.rs` PLUS all 11 new commands from Task 1 (≥ 115 entries in `"denied"`, deduplicated), and include the intentional FR-4 allowances (Python heredoc with markdown backticks + `out.push(line)`, Node template literal + `a.push(1)`, etc.) in `"allowed_on_purpose"`.
- In `tests/hook.rs`: add `hook_guard_replays_the_denied_corpus_and_allowed_exceptions` reading `tests/fixture/hook/denied.json`, asserting `denied.len() >= 115`, `!allowed_on_purpose.is_empty()`, zero overlap between `denied` and `allowed_on_purpose`, every `denied` command satisfies `guard(cmd) == true` and `decide(cmd) == Some(("deny", NEVER_HINT))`, and every `allowed_on_purpose` command satisfies `guard(cmd) == false` and `decide(cmd) == None`.

**Trap**: Keep `tests/fixture/hook/denied.json` completely free of plan-id strings (`PRD-`, `FR-`, `AD-`, `BLOCK-`, `slice-`) so `factory leaks` stays 0 hits without needing any `docs/project.toml` change!

**Acceptance:** `hook_guard_replays_the_denied_corpus_and_allowed_exceptions` passes and fails if any command in `"denied"` is allowed, if any command in `"allowed_on_purpose"` is denied, or if `denied.len() < 115`.

### 3. Add guard-loosening sentences to `skills/{prd,slice}/SKILL.md` and bump version + `skills/bundle.lock` to `0.17.0` — high — blocked by: 2
**Source**: §7 FR-5: "In `skills/prd/SKILL.md` (§2 Acceptance bullet): add one sentence ... In `skills/slice/SKILL.md` (§4 Acceptance bullet): add one sentence ... Bump crate/plugin version from `0.16.0` to `0.17.0` in `Cargo.toml`, `Cargo.lock`, and `mods/cockpit/.claude-plugin/plugin.json`, and regenerate `skills/bundle.lock` via `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle`".

- `skills/prd/SKILL.md` (§2 Acceptance bullet, around line 64): append `"Any FR that loosens a guard (a hook, a deny rule, a validator) carries the acceptance line: everything the old guard denied that this FR does not mean to allow stays denied, proven by replaying a regression corpus."`
- `skills/slice/SKILL.md` (§4 Acceptance bullet, around line 85): append `"When a PRD FR loosens a guard, carry its regression-corpus replay into the Prove task."`
- Bump `0.16.0` → `0.17.0` in `Cargo.toml` (`version = "0.17.0"`), `mods/cockpit/.claude-plugin/plugin.json` (`"version": "0.17.0"`), and update `Cargo.lock` (`cargo check --tests`).
- Run `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` to regenerate `skills/bundle.lock` at `0.17.0`.
- `tests/skills_bundle.rs` (purely additive new test at end of file): add a test calling `assert_unique_passage` on `skills/prd/SKILL.md` for its exact guard-loosening sentence and on `skills/slice/SKILL.md` for its exact Prove-carry sentence.

**Trap**: Run `claude plugin validate mods/cockpit && claude plugin test mods/cockpit` (part of the full gate!) to confirm `mods/cockpit/.claude-plugin/plugin.json` at `0.17.0` stays valid.

**Acceptance:** `bundle_lock_matches`, the new unique-passage assertions in `tests/skills_bundle.rs`, and `claude plugin validate mods/cockpit && claude plugin test mods/cockpit` all pass; deleting either sentence or reverting `skills/bundle.lock` fails them.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §9 QA plan rows FR-4 and FR-5:
- FR-4: `run guard_allows_push_method_calls_and_denies_widened_process_calls` (evidence: test transcript)
- FR-5: `run hook_guard_replays_the_denied_corpus_and_allowed_exceptions` (evidence: test transcript)

**Acceptance:** after the checkpoint, both tests run under `cargo test` with full gate green (including `claude plugin validate mods/cockpit && claude plugin test mods/cockpit`) and transcripts recorded in Progress.
