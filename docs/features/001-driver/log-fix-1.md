# Polish fix 1 — PRD-001

The review findings S1 (with K3 and the second-pass cases), Ln7, K1, C3, Cn2, Cn3, Cn4, L9, L15, L17, Ln6 and S-n2, fixed in `src/worker.rs`, `src/cli/hook.rs`, `src/shell.rs`, `src/paths.rs`, the `# Lookup` text of `src/pack.rs`, `assets/autonomy.md` and `assets/contract-ci-fix.md`. Real Claude Code check: `evidence/polish-f1-deny-real.txt`.

## Decisions

### AD-10001 — How are workers kept off `gh` and `git push`? · polish · 2026-10-10

**Chose:** three layers, each enough for the review's cases on its own where it can see them:
- **Deny rules** `Bash(gh)`, `Bash(gh *)`, `Bash(git push*)` in every worker's `--settings` (`worker::DENY_RULES`): any `gh` and any `git push`, not just `gh pr merge*` and `git push --force*`.
- **The hook's guard** (`cli::hook::guard`, on `shell::script`) reads every simple command in the line at any depth: lists, pipes, groups, `if`/`for`/`case` bodies, `$(…)`, backticks, `<(…)`, heredoc bodies fed to a shell, `sh -c`/`bash -c` strings, `eval`, `env -S`, and wrappers (`env`, `command`, `exec`, `nohup`, `time`, `timeout`, `nice`, `stdbuf`, `sudo`, `xargs`, `find -exec`, `watch` and the rest of its table). It denies any `gh`, and any `git push` (also `send-pack`, `subtree push`, `lfs push`, an inline `-c alias.x=push`) after git's global options (`-c`, `-C`, `--git-dir`, `--no-pager` …). Where the reading is unsure — an unbalanced quote, an interpreter handed inline code or stdin, a shell reading a pipe, a program word that is an expansion — it denies when `gh` or `push` appears as a whole word anywhere in the line; hook input that does not parse is denied on the same test. One reason for every deny: `workers never run gh or git push: the driver pushes the branch and talks to GitHub; …`.
- **No credential:** `GH_TOKEN`, `GITHUB_TOKEN`, `GH_ENTERPRISE_TOKEN` and `GITHUB_ENTERPRISE_TOKEN` are removed from the worker's env after the lane's env is applied, and `GH_CONFIG_DIR` is an empty 0700 `runs/<id>/gh-config`, so a `gh` that gets past both finds no login. `SSH_AUTH_SOCK` is kept.

**Over:**
- Wider prefix rules alone: a rule cannot see `true && gh …` or `git -c k=v push`, which the review proved.
- Taking `gh` off the worker's `PATH`: `/usr/bin/gh` by absolute path walks round it, and the worker's `PATH` is the owner's.
- Scrubbing `SSH_AUTH_SOCK`: it breaks SSH commit signing and SSH dependency fetches. An HTTPS push with a credential helper would not need it anyway.

**Because:** the never-list is held against an agent under pressure, not a malicious one (the brief's threat model). Three layers mean an accidental `gh` or push fails even when one layer misses it. What still gets through: a script file the worker writes and then runs, a git alias from config, a `make` or `npm` target, a wrapper not in the table, and a `git push` through SSH or a credential helper, since only the GitHub tokens and `gh`'s config are withheld. The real Claude Code check refused all three probes (`evidence/polish-f1-deny-real.txt`).
**Undo:** `DENY_RULES`, `GH_TOKEN_VARS` and `gh_config_dir` in `src/worker.rs`; `guard` and its helpers in `src/cli/hook.rs`; `shell::script` in `src/shell.rs`.
**Review:** decide — the worker env and deny rules are the PRD's §4 FR-2 interface, widened; `SSH_AUTH_SOCK` stays.

### AD-10002 — How do the pack and the hook name the `factory` binary? · polish · 2026-10-10

**Chose:** `paths::factory_command()`: the running binary's absolute path (`current_exe`, with Linux's ` (deleted)` suffix stripped after a rebuild, single-quoted when it holds a shell character), when its file name is `factory`. Otherwise it is the bare `factory`: a test binary that builds a pack in-process is not the CLI. The `# Lookup` verbs (`pack::lookup`), the grep hint and the settings' hook command all use it.
**Over:** the bare name, with `qa-guide.md` requiring an installed `factory`. CP-7's real worker followed the hint and got exit 127.
**Because:** the worker should run the same build that wrote its pack.
**Undo:** `factory_command`/`self_exe` in `src/paths.rs`, `pack::lookup`, `hook::grep_hint`.
**Review:** no

### AD-10003 — What is "the repo" a held file is relative to? · polish · 2026-10-10

**Chose:** `FACTORY_REPO`, which the driver sets to the run's repo for every worker. Without it (the hook run by hand), it is the call's `cwd`. `Held::section` matches `<repo>/<held path>` exactly. Recursive greps are judged against the same root.
**Over:** `CLAUDE_PROJECT_DIR`, which belongs to whatever session started the driver and is scrubbed from workers (AD-10007).
**Because:** a worker that runs `cd src` is still in the repo, and another checkout with the same relative path is not the pack's file (C3).
**Undo:** `REPO_ENV` in `src/worker.rs`, `hook()`/`hook_with` in `src/cli/hook.rs`.
**Review:** no

### AD-10004 — How does `autonomy.md` square "read whole" with AD-1304? · polish · 2026-10-10

**Chose:** in `assets/autonomy.md`, line 67 now reads "whole when it fits … unless your pack already holds it: then read only the range you need, because a whole read of a file in your pack is refused". In the same pass, three lines now say what the hook enforces:
- line 58, in the Never list: a worker runs no `git push` and no `gh` at all;
- line 78: the driver does the flaky rerun;
- line 85: the driver commits and pushes a block, and a worker stops after step 2.

`assets/contract-ci-fix.md` lines 5 and 13 say the driver fetched the failed log and that a worker never runs `gh`. Every change is made in place, so the file keeps 100 lines and the `autonomy.md:NN` citations in `src/ci.rs`, `src/scheduler.rs` and the PRD still point at the same lines.
**Over:** loosening AD-1304 to allow whole reads, or adding lines.
**Because:** without this, the pack tells a worker to do what its own hook refuses (Cn2), and to run `gh run rerun` and `git push`.
**Undo:** the asset lines.
**Review:** fyi — `assets/autonomy.md` is no longer byte-identical to `~/.claude/skills/feature-docs/autonomy.md`. It is this repo's own copy (slice-03), and line 67 may be worth taking upstream.

### AD-10005 — When is a grep's operand a directory, and where is it resolved? · polish · 2026-10-10

**Chose:**
- **Cn4:** an operand is a directory when `Path::is_dir` says so and it is under the repo, or when it is a glob whose last part is `*` or `**`. A path that does not exist is not a directory. This replaces AD-1305's lexical rule, where a trailing `/` or a last segment with no extension meant a directory.
- **Cn3:** each leading `cd X &&` moves the directory later operands resolve against. A target the hook cannot follow (`cd`, `cd -`, `cd ~`, `cd "$D"`) means no decision.

**Over:** the lexical rule, which denied `grep -rn x Makefile` and passed `grep -rn x src.d`.
**Because:** one `stat` per operand costs microseconds against AD-1305's 20 ms budget, and the deny is a hint, so a missed one costs only tokens.
**Undo:** `repo_dir` and `bash` in `src/cli/hook.rs`, `shell::first_command_in`.
**Review:** no

### AD-10006 — How does a worker's stream reach its file? · polish · 2026-10-10

**Chose:** the worker's stdout is `runs/<id>/workers/<session>.jsonl` itself (`Stdio::from(File)`), with no tee thread. The driver parses the file when the process exits (`summary_at`).
**Over:** keeping the tee and ignoring `EPIPE`.
**Because:** the stream must outlive a driver that dies: a tee leaves the worker writing into a pipe nobody reads (L9). Also, a grandchild holding the pipe could stall the reader's join. `tests/worker.rs::the_stream_outlives_the_driver` kills only the driver and finds the worker's `result` line on disk.
**Undo:** `Worker::spawn`/`exit` in `src/worker.rs`.
**Review:** no

### AD-10007 — Which `CLAUDE_*` variables does a worker keep? · polish · 2026-10-10

**Chose:**
- **Kept:** `worker::KEEP_CLAUDE_VARS` and every `CLAUDE_CODE_USE_*` (Bedrock, Vertex, Foundry). The kept list is `CLAUDE_CONFIG_DIR`, `CLAUDE_CODE_OAUTH_TOKEN`, `CLAUDE_CODE_API_KEY_HELPER_TTL_MS`, `CLAUDE_CODE_SKIP_{BEDROCK,VERTEX,FOUNDRY}_AUTH` and `CLAUDE_CODE_CLIENT_{CERT,KEY,KEY_PASSPHRASE}`.
- **Scrubbed:** `CLAUDECODE` and every other `CLAUDE_*`.
- **Untouched:** `ANTHROPIC_*`, as before.

**Over:** scrubbing only a named list of session variables, which would leak any session variable a later Claude Code adds.
**Because:** the account, config dir and provider must reach the worker, or it logs in as someone else or not at all (L17). The session's variables must not reach it.
**Undo:** `KEEP_CLAUDE_VARS`/`scrubbed` in `src/worker.rs`.
**Review:** fyi — a provider or auth variable outside the list is dropped; add it there.

### AD-10008 — Which files and directories are private? · polish · 2026-10-10

**Chose:**
- The data dir is created 0700 only when it is missing. An existing one keeps its mode, since `FACTORY_DATA_DIR` may name a directory the owner shares on purpose.
- `workers/` and `gh-config/` are 0700.
- A worker's stream, stderr and settings files are 0600.

**Over:** a `chmod` of existing directories.
**Because:** streams hold the worker's whole transcript (S-n2).
**Undo:** `private_dir_all` in `src/paths.rs`, `private_file` in `src/worker.rs`.
**Review:** no — `runs/` and `runs/<id>/` are created by `src/run.rs`, outside this fix's files, and keep the umask's mode.

### AD-10009 — Where do paths become absolute? · polish · 2026-10-10

**Chose:** `paths::data_dir()` and `config_dir()` resolve against the driver's cwd. `Worker::spawn` makes the run dir and the pack path absolute once more before they go on the command line or into `FACTORY_PACK`.
**Over:** absolutizing only the worker's flags, which Ln6 named. A relative `FACTORY_DATA_DIR` also broke the checkpoint's `git read-tree` into `runs/<id>/gate-index`, run in the repo. `tests/worker.rs` caught this from a subdirectory.
**Because:** every consumer of the data dir runs somewhere else: the worker, `git`, the gate.
**Undo:** `dir` in `src/paths.rs`.
**Review:** no — the detached driver (`src/daemon.rs`) still inherits a relative `FACTORY_DATA_DIR` and runs in the repo; it should be handed the absolute one.

### AD-10010 — What does the no-merge source scan look for? · polish · 2026-10-10

**Chose:** `tests/closeout.rs::merges_or_readies` reads each `src/**/*.rs` file in two ways. With whitespace runs collapsed, it looks for `pr merge`, `pr ready` and `/merge` (the REST endpoint). With whitespace removed, so that rustfmt's one-argument-per-line arrays read as one, it looks for `"pr","merge"`, `"pr","ready"`, the GraphQL `mergePullRequest`, `markPullRequestReadyForReview` and `enablePullRequestAutoMerge`, and a `"pr","create"` array without `"--draft"`. The scan has no exemption: the deny rules name no merge, so the stale strip of `"Bash(gh pr merge*)"` is gone. The test also checks the scan itself against each form and against `ci::pr_create`'s shape (K3).
**Over:** keeping the substring scan, which missed the args-array form the driver actually uses for `gh`.
**Because:** this keeps the test's intent, that no code path in the driver merges a PR or marks one ready, and makes it see how the driver really calls `gh`.
**Undo:** the test.
**Review:** no
