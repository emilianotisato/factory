# Slice 01 — Hook judges scripts and greps

**Source**: [`prd.md`](prd.md) → §3 FR-3, FR-4. **Depends on**: — (independent of every other slice; slice 05 waits on this one, because it adds the ctx answer to the same deny path and edits the same `assets/lookup.md`)
**Created**: 2026-10-15

**Progress**:
- **Built**: `interpreter()` takes the command's stdin and reads the script where it answered Unsure: the word after an inline flag (or its attached value, `-cCODE`, `--eval=CODE`), the first non-flag word after `deno eval`, and each stdin document for `-` or no script argument. `script()` is Forbidden only for a process call (`process_call()`, the FR-3 list as written) plus a gh/push mention, else Fine; an empty stdin or a bare inline flag stays Unsure. `grep_search()`/`rg_search()` return a `Search` of patterns (first positional, each `-e`/`--regexp` value attached or not, `None` for `-f`/`--file`) and paths; `bare_name()` denies one identifier, bare or in `\b…\b`/`\<…\>`, over the repo with no searched path under a `VENDOR_DIRS` component. The hint and `assets/lookup.md` say the new rule and name `--vendor`. Unit fixture `repo()` gains `vendor/acme/Recon.php`.
- **Tests**: `only_a_bare_name_outside_vendor_code_goes_to_ctx` (unit); `rg -n 'a b' src` and `egrep -r 'a|b' ./` moved to the allowed list; `a_script_is_denied_only_when_it_runs_a_process_and_names_gh_or_push` (`tests/hook.rs`, through `guard` and `decide`); `tests/cli.rs` pins the new hint; `tests/loop.rs` asserts the new Lookup rule and the old one's absence. `doubt_is_denied_when_it_names_gh_or_push` and `the_rest_gets_no_decision` unchanged and green.
- **Inner loop**: 1 run, 0 red, ~2 min.
- **Deviations**: attached inline values are read too (AD in the log).
- **Prove**: FR-3 and FR-4 transcripts are left for the checkpoint.
- **Checkpoint 1, FR-3**: `factory hook` allows the `line.push(` heredoc and `print('push')`, denies the subprocess, `os.system` and `execSync` scripts with the never-hint — cp-1/fr3-hook-scripts.txt
- **Checkpoint 1, FR-4**: `factory hook` in a fixture repo with `vendor/` allows the literal, regex and vendor greps, denies the three bare-`Recon` greps with the hint naming `--vendor` — cp-1/fr4-hook-greps.txt

**Slice**: The hook stops denying two kinds of call the owner's runs showed should run. An interpreter script that only mentions gh or push runs, unless it also makes a process call. A recursive grep or rg is denied only for a bare identifier outside vendor code, so literals, regex, several patterns and vendor paths pass. The hint and the pack's `# Lookup` state the new rule. The denial's text is still the hint alone: the ctx answer under it, the `def` trims and the header change are slice 05's.

## Current code / environment

- **`forbidden()` denies any doubt that mentions gh or push**: `src/cli/hook.rs:173-187` ends `unsure && mentions(text)` over the whole command text, and `mentions()` (`:199-210`) matches `gh` or `push` as a whole word anywhere. One Unsure command and a `line.push(` in its heredoc deny the whole call with `NEVER_HINT` (`:14`).
- **An interpreter never sees its script**: `reach()` (`:221`) falls through to `p => return interpreter(p, args)` (`:316`). `interpreter()` (`:514-549`) takes no `stdin`. It answers Unsure for an inline flag (`-c`, `-e`, `-p`, `--eval`, `-r`, …; the script is the next word), for `-` or for no script argument (the script is on stdin), and for `deno eval`. A non-flag argument is a script file and answers Fine.
- **The pattern to follow**: `shell_call()` (`:464-511`) already passes `stdin` and judges each here-document with `forbidden(doc, depth + 1)`, and a `-c` line with `nested()` (`:191-197`).
- **A piped script stays a doubt**: the parser fills an interpreter's stdin only from a here-document or a here-string (`src/shell.rs:418`, `:455`). So `printf '…' | python3` has an empty stdin, no script to read, and keeps today's rule. So do unbalanced quoting and nesting past `MAX_DEPTH` (`:164`, `:174-176`).
- **The grep deny never sees the pattern**: `bash()` (`:142-145`) denies grep, egrep, fgrep and rg whenever `over_repo(grep_paths(args) …)` holds. `grep_paths()` (`:561-629`) and `rg_paths()` (`:632-705`) drop the first positional (`positional.remove(0)`), skip the value of `-e`/`--regexp` (`i += 1`), and keep only a `pattern_given` flag. An attached value (`-eRecon`, `--regexp=Recon`) is the rest of the word; the short-cluster loop breaks at the first value-taking letter (`e f m A B C d D` for grep, `SHORT_ARG` `"efgtTmABCjMrEd"` for rg).
- **`VENDOR_DIRS`** is a `pub const` at `src/ctx/walk.rs:8`: `vendor`, `node_modules`, `target`.
- **`repo_dir()` matches real directories only** (`src/cli/hook.rs:708-719`, or a `*`/`**` glob). The unit fixture `repo()` (`:831-848`) makes `src/Http`, `app`, `storage/logs` and `docs/features/001-x`, and no `vendor/`. Without one, `grep -rn Recon vendor/` is allowed because `vendor/` is no directory, and the vendor rule is never tested.
- **The hint**: `grep_hint()` (`:16-21`): "use: {factory} ctx refs <word> (or ctx def <symbol>); grep only for literal strings and log output". `assets/lookup.md:7` says "The one rule: grep only for literal strings and log output." and `:9` says a recursive grep or rg over the repo is answered with the hint. `:3` (def "with its body") is slice 05's; leave it.
- **Tests that pin the old rule change with it.** The PRD names them, which makes the change no weakening (`assets/autonomy.md:54`):
  - `recursive_grep_over_the_repo_is_denied_with_the_verb` (`src/cli/hook.rs:881-915`): `rg -n 'a b' src` and `egrep -r 'a|b' ./` move to the allowed list. Every `foo` case stays denied, `grep --recursive -e foo src/Http` included.
  - `tests/cli.rs:100-107` pins the hint's exact text. Its fixture's `src/` is empty.
  - `tests/loop.rs:412` asserts the slice pack's Lookup holds "grep only for literal strings and log output".
- **Tests that must stay green as they are**: `tests/hook.rs` `doubt_is_denied_when_it_names_gh_or_push` (`:146-162`). Its python and node cases make a process call (`os.system`, `execSync`) and stay Forbidden; its `printf … | bash`, `printf … | sh` and `cat cmds | sh # gh` cases are piped. `the_rest_gets_no_decision` (`:166-190`) and the other `tests/hook.rs` tests stay as they are. `assert_denied` checks both `guard(cmd)` and `decide`, so the two must agree.
- **Deliberate non-change**: *prd §7: "`git grep` and a grep that is not the first command stay unjudged (935 `git grep` uses). Catching them means forcing ctx, which the replay rejects."* `git grep -n foo` stays in the allowed list (`src/cli/hook.rs:908`).
- **Deliberate non-change**: the header's no-I/O promise (`:1`) still holds after this slice: everything here reads the command text and a `stat`. Slice 05 changes it.

## Tasks

### 1. Read an interpreter's script — high — blocked by: —
**Source**: §3 FR-3: "When the doubt is an interpreter's script, its stdin or here-document or its inline-flag argument, the hook reads that script. It is Forbidden only when the script both makes a process call and mentions gh or push; otherwise it is Fine."

`src/cli/hook.rs`: `reach()` (`:316`) passes `stdin` to `interpreter()`. Where `interpreter()` answers Unsure today, it judges the script instead: the word after an inline flag, the argument of `deno eval`, or, for `-` or no script argument, each stdin document. Forbidden when the script holds a process call and `mentions()` it; Fine otherwise. An empty stdin, or an inline flag with no word after it, stays Unsure.

A process call is, from FR-3: `subprocess`, `os.system`, `os.popen`, `os.exec…`, `os.spawn…`, `pty.spawn`, `child_process`, `execSync`, `spawnSync`, `IO.popen`, `Open3`, `shell_exec`, `passthru`, `proc_open`, a backtick, `%x`, `qx`, or a bare `system(`, `exec(`, `spawn(` or `popen(` not preceded by `.`. Use this list as written; no parser per language (§3 FR-3 Rejected).

**The trap:** `forbidden()` (`:186`) still ends `unsure && mentions(text)` over the whole command. The script must answer Fine or Forbidden, never Unsure, or the mention in it denies the command anyway.

**Acceptance:** hook tests, each through both `guard` and `decide`:
- allowed: a `python3 - <<'EOF'` heredoc that edits a file, with `line.push(` and `git` in its text and no process call; `python3 -c "print('push')"`.
- denied with `NEVER_HINT`: a python heredoc calling `subprocess.run(["gh", "pr", "view"])`; `python3 -c "import os; os.system('git push')"`; `node -e "require('child_process').execSync('gh pr list')"`; `printf 'import os; os.system("git push")' | python3`, piped, so there is no script to read.
- `doubt_is_denied_when_it_names_gh_or_push` and `the_rest_gets_no_decision` pass unchanged.

### 2. Deny a grep only for a bare identifier outside vendor code — high — blocked by: —
**Source**: §3 FR-4: "The hook denies only when the pattern is a single identifier, `[A-Za-z_][A-Za-z0-9_]*`, alone or as a whole word (`-w`, `\b…\b`, `\<…\>`), and no searched path lies under a `VENDOR_DIRS` dir (`src/ctx/walk.rs:8`: `vendor`, `node_modules`, `target`). Literals, regex, several patterns and vendor paths pass."

`src/cli/hook.rs`: `grep_paths()` and `rg_paths()` also return the patterns they saw: the first positional, or each `-e`/`--regexp` value, attached or not. A `-f`/`--file` pattern is unknown, so it passes. `bash()` (`:142-145`) denies only when there is exactly one pattern, it is an identifier bare or wrapped in `\b…\b` or `\<…\>`, the search is over the repo as today, and no searched path has a `VENDOR_DIRS` component under the repo root. With no path given, the searched path is the cwd after the `cd`s (`here`).

**The trap:** `repo()` (`:831-848`) has no `vendor/`, so give it one holding a file, or the vendor case passes for nothing.

**Acceptance:** hook unit tests in `repo()`:
- allowed: `grep -rn "fn main" src`, `grep -rn 'foo\|bar' src`, `grep -rn Recon vendor/`, and `rg -n 'a b' src` and `egrep -r 'a|b' ./`, which move out of the denied list of `recursive_grep_over_the_repo_is_denied_with_the_verb`.
- denied with the hint: `grep -rn Recon src`, `grep -rnw Recon src`, `rg -n '\bRecon\b' src`, `grep -rn '\<Recon\>' src`, and every `foo` case already in that denied list.

### 3. Say the new rule — high — blocked by: 2
**Source**: §3 FR-4: "The texts that state the old rule say the new one: a bare name goes to ctx, anything else may be grepped. They are `grep_hint()` … and the pack's `# Lookup` (`assets/lookup.md:7`, `:9`)." and "The hint adds `--vendor` for vendor code."

`src/cli/hook.rs:16-21` `grep_hint()`, `assets/lookup.md:7` and `:9`. The hint keeps the absolute binary path and both verbs, adds `--vendor` for vendor code, and says a bare name goes to ctx and anything else may be grepped. The Lookup's one rule and its hint sentence say the same.

**Acceptance:** `tests/cli.rs:100-107` pins the new hint's exact text, which names `--vendor`. The slice-pack test at `tests/loop.rs:412` asserts the pack's `# Lookup` states the new rule and no longer holds "grep only for literal strings and log output".

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
`prd.md` § QA plan rows, driven below the UI:
- **FR-3**: run `factory hook` fed the heredoc, inline and subprocess scripts of §3 FR-3; keep the transcript.
- **FR-4**: run `factory hook` fed the greps of §3 FR-4 in a fixture repo with a `vendor/` dir; keep the transcript.

**Acceptance:** after the checkpoint, each row has its transcript in this slice's **Progress**.
