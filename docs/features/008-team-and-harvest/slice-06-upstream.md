# Slice 06 — factory upstream

**Source**: [`prd.md`](prd.md) → §3 FR-8. **Depends on**: 02 (the home-path hit is 02's matcher)
**Created**: 2026-10-15

**Progress**:
- Built: `src/upstream.rs` (draft parse; hits for a home path, the repo's dir name and its origin's owner and name as whole words, an email; the `gh issue list` search), `src/cli/upstream.rs` (`factory upstream check|post`, both refusing when `FACTORY_BOARD` is set), `tests/upstream.rs` (the five cases plus `post` refusing a hit).
- Inner loop: 3 runs, 2 red, 3 min. Red 1: clippy's `cloned_ref_to_slice_refs`. Red 2: the stub `gh` logs quoted argv, and the fixture's git has no author, so the test sets one.
- Deviation: `post` reruns the leak check but not the issue search (AD in the log).
- Prove (CP-2) FR-8: `evidence/cp-2-fr8-upstream.txt`, the tests' transcript, then the branch binary on a scratch client repo with a GitHub-host origin against the stub `gh`: `check` names a home path, the dir name, the owner and an email (exit 1) and searches once; `post` refuses a hit; a clean `post` without `[source]` calls `issue create --repo emilianotisato/factory`; with `[source]`, one committed idea and no `gh` call; a failed search is one line, exit 0; with `FACTORY_BOARD` set, both refuse and `gh.log` stays empty.

**Slice**: After this slice, `factory upstream check <draft>` reports what would leak from a client repo into the public `emilianotisato/factory`: a home path, the repo's name or its origin's owner or name, an email. It then lists up to five open issues matching the draft's title. `factory upstream post <draft>` checks again, then commits the draft as an idea in the `[source]` checkout when one is set, or opens an issue with `gh`. Both refuse inside a run. The skill that writes the draft in general terms and shows it to the person is slice 08's `/harvest`.

**Highest risk**: the stub `gh` answering nothing. `issue list` and `issue create` print nothing and exit 0, so a test that reads stdout proves nothing: assert on `fx.gh_log()`.

## Current code / environment

- **A verb with subcommands**: `Review { #[command(subcommand)] verb: cli::review::ReviewVerb }` (`src/main.rs:37-40`), dispatched at `:85` as `cli::review::review(&verb, &mut stdout(), &mut stderr())`. Modules are listed in `src/cli/mod.rs:2-16`, one per subcommand, returning an exit code.
- **The run's marker**: `BOARD_ENV = "FACTORY_BOARD"` (`src/worker.rs:55`), set for every worker (`:243`). `src/cli/docs.rs:254` reads it the same way: `var_os(…).filter(|v| !v.is_empty())`. The run's hook refuses `gh` with `NEVER_HINT` (`src/cli/hook.rs:14`); word the refusal in its spirit, naming the driver as the one that talks to GitHub.
- **The names to catch**: the repo dir name is `paths::repo_root(cwd)` (`src/paths.rs:104`) plus `file_name()`. The origin is `git remote get-url origin`, through `preflight::git` (`src/docs/preflight.rs:12`, returns `Option<String>`). Parse owner and name only from host forms (`git@host:owner/name(.git)`, `https://host/owner/name(.git)`). The fixtures' origin is a local bare path, which has neither.
- **The search**: `ci::gh(repo, args)` (`src/ci.rs:79`) returns `Result<Output>`. A spawn error or a non-zero exit is "search failed", one line, not a hit. Empty stdout is no match.
- **The idea**: `config::load(&paths::config_dir())` (`src/config.rs:135`) gives `config.source: Option<SourceRepo { path }>` (`:19,25-27`). `ideas::add(root, &Idea)` (`src/docs/ideas.rs:173`) writes `docs/ideas/<id>.md` and returns the id; `Idea` (`:21-29`) takes `title`, `state`, `added` (`scaffold::today()`), `origin`, `retrigger`, `priority` and `body`. `crate::commit::commit(repo, &[path], msg)` (`src/commit.rs:36`) stages exact names and commits; outside a run nothing holds a branch, so it does not refuse.
- **The gh stub**: `tests/stubs/gh` logs every call's argv to `gh.log` (`fx.gh_log()`, `tests/harness/mod.rs:415-421`), answers `issue …` with nothing and exit 0 (`:249-250`), and fails a call whose argv starts with a line of `scenario/gh-fail` in the cwd, once (`:76-84`). The real `gh` is on this machine (`/usr/bin/gh`): never take the stub dir off `PATH`.
- **The test env**: `FACTORY_CONFIG_DIR` points at a fixture dir (`tests/harness/mod.rs:540`); write `[source]` into its `config.toml`, as `source_repo` does (`tests/closeout.rs:2071-2095`). Copy that helper rather than share it: slice 01 rewrites it. A run test's env may carry `FACTORY_BOARD`; clear it with `.env_remove(…)` on every success path (`tests/cli.rs:36` does this for `FACTORY_REPO`).
- **The fixture's name**: the fixture repo's directory is `repo`, so a "clean" draft must not hold the word.
- **Deliberate non-changes**: *(prd §7 "Not handled: a +1 comment from `factory upstream`.")* No `--on N`. *(prd §3 FR-8 accepted cost: "The name check is a floor.")* No customer-name list, no secret patterns beyond the four hits.

## Tasks

### 1. The check, in the library — high — blocked by: —
**Where**: a new `src/upstream.rs`, registered in `src/lib.rs`.

**Source**: §3 FR-8: "The draft is a markdown file: its first `# ` line is the title, the rest the body. **`factory upstream check <draft>`** exits 1 with one line per hit for: an absolute home path; the current repo's directory name, or its `origin` remote's owner or name; an email address. It then lists up to five open issues on `emilianotisato/factory` matching the title (`gh issue list --state open --search`), so the person can comment on a match by hand instead of posting a repeat. A failed search (no `gh`, no login) prints one line and is not a hit."

A function that takes the draft text and the repo dir and returns the hits, each with its line, and one that runs the search. Names match case-insensitively as whole words. The search is `gh issue list --repo emilianotisato/factory --state open --search <title> --limit 5 --json number,title,url`.

The trap: never take a name from a local origin path, or the fixture's temp dir names become "hits".

**Acceptance:** unit tests: a home path, the dir name, an owner and a name from both host forms, an email, a local-path origin giving no name hit, a word that only contains the name not hitting.

### 2. The verb — high — blocked by: 1
**Where**: `src/main.rs:37-40,85`, `src/cli/mod.rs`, a new `src/cli/upstream.rs`.

**Source**: §3 FR-8: "**`factory upstream post <draft>`** runs the check again and refuses on a hit. With `[source] path` set to a factory checkout, it adds the draft as an idea there and commits only that file. Otherwise it runs `gh issue create`." And: "Both refuse when `FACTORY_BOARD` is set (`src/worker.rs:55`): never inside a run, where the driver owns every GitHub call."

`Upstream { verb: UpstreamVerb }` with `Check { draft }` and `Post { draft }`. Both refuse first when `FACTORY_BOARD` is non-empty. `check` prints the hits, then the matches, and exits 1 on a hit. `post` runs the check, exits 1 on a hit, then with `[source]` adds the idea (title from the draft, a generic origin such as `factory upstream`) and commits that one file in the checkout; without it, runs `gh issue create --repo emilianotisato/factory --title <title> --body <body>`. It prints what it did: the idea's path, or the issue call's output.

**Acceptance:** task 3's tests.

### 3. The tests — high — blocked by: 2
**Where**: a new `tests/upstream.rs`.

**Source**: §3 FR-8 acceptance: "Against the stub `gh` (`tests/stubs/gh`): a draft holding `/home/x/` or the repo name exits 1 naming the hit; a clean draft with no `[source]` calls `gh issue create --repo emilianotisato/factory`; with `[source]` set, one committed idea appears in that checkout and `gh` creates no issue; with `FACTORY_BOARD` set, both refuse."

- `check` on a draft holding `/home/x/`: exit 1, the hit named; the same for the word `repo`; `gh.log` holds one `issue list` call;
- `post` of a clean draft, no `[source]`: exit 0, `gh.log` holds `issue create --repo emilianotisato/factory`;
- `post` with `[source]` set: one new committed file under the checkout's `docs/ideas/`, a clean `git status` there, and no `issue create` in `gh.log`;
- `scenario/gh-fail` holding `issue list`: `check` on a clean draft exits 0 and says the search failed;
- `FACTORY_BOARD` set: `check` and `post` both refuse, and `gh.log` is empty.

**Acceptance:** the five cases above, in tests named for the behavior.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §5 this slice proves, below the UI:

- **FR-8**: "run: `factory upstream check` and `post` against the stub `gh`, with and without `[source]`, and with `FACTORY_BOARD` set" — task 3's test transcript.

**Acceptance:** after the checkpoint, Progress names the transcript.
