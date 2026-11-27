# Slice 04 — Upstream pull triage verb

**Source**: [`prd.md`](prd.md) → §5 (FR-3). **Depends on**: Slice 03 (same files: `src/upstream.rs`, `src/cli/upstream.rs`, `tests/upstream_hardening.rs`, and reuses `ci::gh_stdin` and host-qualified `upstream::REPO`)
**Created**: 2026-11-27

**Progress**: `PulledIssue` + `list_open`/`view_issue`/`comment_issue`/`close_issue`; `UpstreamVerb::Pull` with list/`--json`/`--idea`/`--fold`/`--drop`; `upstream_pull_lists_and_triages_issues_to_idea_fold_and_drop`. Inner loop: 2 runs, 1 red, ~2 min — clippy `from_ref` for commit pathspecs. Deliberate red: dropped `createdAt` rename → unit test failed on `created_at` empty; dropped `#{}` list prefix → integration test failed on `#42` missing. cp-2/FR-3/upstream-pull.txt · cp-2/FR-4/home_paths.txt · cp-2/FR-4/docs-check.txt

**Slice**: Adds `factory upstream pull` to list open upstream issues from `github.com/emilianotisato/factory` (human-readable or `--json`) and triage any issue into `docs/ideas/` via `--idea <n>`, `--fold <n> <idea_id>`, or `--drop <n> --because <reason>`, always committing local changes before posting the stdin-piped comment and closing the issue on GitHub.

## Current code / environment

- **`src/upstream.rs`** (after Slice 03): Exports `pub const REPO: &str = "github.com/emilianotisato/factory";` and `Issue { number, title, url }`.
- **`src/ci.rs`** (after Slice 03): Exports `ci::gh(repo, args)` and `ci::gh_stdin(repo, args, stdin_bytes)`.
- **`src/cli/upstream.rs`** (after Slice 03): `UpstreamVerb` has `Check` and `Post`; `upstream()` already checks `worker::BOARD_ENV` at entry before dispatching any verb.
- **`src/docs/ideas.rs:14, 21, 173, 303`**:
  - `ideas::DIR` = `"docs/ideas"`.
  - `ideas::Idea { title, state, added, origin, retrigger, priority, body }`.
  - `ideas::add(root: &Path, idea: &Idea) -> Result<String>` creates `docs/ideas/{id}.md` via `O_EXCL` and returns `id`.
  - `ideas::locate(root: &Path, id: &str) -> Result<(String, String)>` validates and resolves an existing idea id to `(id, rel)` (`docs/ideas/{id}.md`).
- **`src/docs/scaffold.rs`**: `scaffold::today()` returns today's UTC date (`YYYY-MM-DD`).
- **`src/commit.rs:105`**: `commit::commit(repo: &Path, paths: &[String], message: &str) -> Result<Option<Committed>>` commits by explicit pathspec.
- **`tests/upstream_hardening.rs`** (created in Slice 03): Has a tempdir executable `gh` stub pattern that logs argv and stdin cleanly.
- **Deliberate non-change**: Do NOT add interactive TTY prompts (`--idea`, `--fold`, `--drop` are explicit non-interactive flags usable by both humans and skills, per prd §5). Commenting uses `gh issue comment --body-file -` via `ci::gh_stdin` followed by `gh issue close` (not `gh issue close --comment`, keeping comment bodies off argv per prd §5 Accepted cost).

## Tasks

### 1. Add GitHub issue list/view/comment/close helpers in `src/upstream.rs` — high — blocked by: —
**Source**: §5 FR-3: "Queries open issues on `github.com/emilianotisato/factory` via `gh issue list --repo github.com/emilianotisato/factory --state open --limit 50 --json number,title,body,url,labels,createdAt,author`... fetches issue `#<NUMBER>` (`gh issue view <NUMBER> --repo github.com/emilianotisato/factory --json number,title,body,url`)... posts a comment via `ci::gh_stdin` (`gh issue comment <NUMBER> --repo github.com/emilianotisato/factory --body-file -`)... closes via `gh issue close <NUMBER> --repo github.com/emilianotisato/factory --reason completed` / `--reason \"not planned\"`."

`src/upstream.rs`.
- Add `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]` structs for pulled issues:
  - `PulledAuthor { login: String }` (optional / default)
  - `PulledLabel { name: String }`
  - `PulledIssue { number: u64, title: String, #[serde(default)] body: String, url: String, #[serde(default)] labels: Vec<PulledLabel>, #[serde(default, rename = "createdAt")] created_at: String, #[serde(default)] author: Option<PulledAuthor> }` (note: keep JSON field names faithful when serialized for `--json`).
- Add functions:
  - `pub fn list_open(repo: &Path) -> Result<Vec<PulledIssue>>` calling `ci::gh(repo, &["issue", "list", "--repo", REPO, "--state", "open", "--limit", "50", "--json", "number,title,body,url,labels,createdAt,author"])`.
  - `pub fn view_issue(repo: &Path, number: u64) -> Result<PulledIssue>` calling `ci::gh(repo, &["issue", "view", &number.to_string(), "--repo", REPO, "--json", "number,title,body,url"])`.
  - `pub fn comment_issue(repo: &Path, number: u64, body: &str) -> Result<()>` calling `ci::gh_stdin(repo, &["issue", "comment", &number.to_string(), "--repo", REPO, "--body-file", "-"], body.as_bytes())`.
  - `pub fn close_issue(repo: &Path, number: u64, reason: &str) -> Result<()>` calling `ci::gh(repo, &["issue", "close", &number.to_string(), "--repo", REPO, "--reason", reason])`.
- On any non-zero `gh` exit, return `Err(Error::Tool { cmd, detail: out.text() })`.

**Acceptance:** Unit tests in `src/upstream.rs` verify JSON deserialization/serialization of `PulledIssue` (with empty labels/body and populated author/createdAt) (fails if schema fields or `#[serde(rename = "createdAt")]` are broken).

### 2. Add `UpstreamVerb::Pull(PullArgs)` and triage actions in `src/cli/upstream.rs` — high — blocked by: 1
**Source**: §5 FR-3: "Add `UpstreamVerb::Pull(PullArgs)`: `factory upstream pull` (default / list mode)... `--json`... `--idea <NUMBER>`... `--fold <NUMBER> <IDEA_ID>`... `--drop <NUMBER> --because <REASON>`... Refuses inside a run (`BOARD_ENV`, same as `check`/`post`). `--idea` and `--fold` require running inside a git checkout that has `docs/ideas/` and refuse if `docs/ideas/` is missing; the local git commit runs BEFORE commenting/closing on GitHub so a failed local commit never closes an issue prematurely. Mutually exclusive flags (`--idea`, `--fold`, `--drop`, `--json` via clap `conflicts_with`)."

`src/cli/upstream.rs`, `tests/upstream_hardening.rs`.
- Define `#[derive(Args, Debug, Clone)] pub struct PullArgs`:
  - `#[arg(long, conflicts_with_all = ["idea", "fold", "drop"])] pub json: bool`
  - `#[arg(long, value_name = "NUMBER", conflicts_with_all = ["json", "fold", "drop"])] pub idea: Option<u64>`
  - `#[arg(long, num_args = 2, value_names = ["NUMBER", "IDEA_ID"], conflicts_with_all = ["json", "idea", "drop"])] pub fold: Option<Vec<String>>`
  - `#[arg(long, value_name = "NUMBER", requires = "because", conflicts_with_all = ["json", "idea", "fold"])] pub drop: Option<u64>`
  - `#[arg(long, value_name = "REASON", requires = "drop")] pub because: Option<String>`
- Add `Pull(PullArgs)` to `UpstreamVerb` and dispatch in `upstream()` (automatically inheriting the `BOARD_ENV` refusal at the top of `upstream()`).
- Implement the 4 modes:
  1. **List (`json` or default)**: call `upstream::list_open(repo)?`. If `json`, print `serde_json::to_string_pretty(&issues)` to `out`. Else if empty print `no open upstream issues`, else print `#{n}  {title}  {url}` per issue.
  2. **`--idea <NUMBER>`**:
     - Refuse if `!repo.join(ideas::DIR).is_dir()` (before calling `gh` or mutating anything).
     - Fetch `issue = upstream::view_issue(repo, number)?`.
     - Create idea via `ideas::add(repo, &ideas::Idea { title: issue.title.clone(), state: "open".into(), added: scaffold::today(), origin: Some(issue.url.clone()), retrigger: None, priority: None, body: issue.body.clone() })?`.
     - Commit by pathspec: `commit::commit(repo, &[rel], &format!("docs: add idea {id} from #{number}"))?` (on commit error, remove the created file and return `Err` BEFORE any GitHub comment/close!).
     - Call `upstream::comment_issue(repo, number, &format!("Tracked as `{rel}`."))?` and `upstream::close_issue(repo, number, "completed")?`.
     - Print confirmation naming `{rel}` and `#{number}`.
  3. **`--fold <NUMBER> <IDEA_ID>`**:
     - Parse `<NUMBER>` as `u64` (clear error on non-number).
     - Refuse if `!repo.join(ideas::DIR).is_dir()`.
     - Resolve existing idea via `let (id, rel) = ideas::locate(repo, &idea_id)?;`.
     - Fetch `issue = upstream::view_issue(repo, number)?`.
     - Append `\n## From #{number} ({url}) ({today})\n\n### {title}\n\n{body}\n` (trimmed cleanly, preserving existing trailing newline) to `repo.join(&rel)`.
     - Commit by pathspec: `commit::commit(repo, &[rel], &format!("docs: fold #{number} into idea {id}"))?` (on commit error, restore the previous file contents and return `Err` BEFORE any GitHub comment/close!).
     - Call `upstream::comment_issue(repo, number, &format!("Folded into `{rel}`."))?` and `upstream::close_issue(repo, number, "completed")?`.
     - Print confirmation naming `{rel}` and `#{number}`.
  4. **`--drop <NUMBER> --because <REASON>`**:
     - Validate `!because.trim().is_empty()` (refuse if blank).
     - Call `upstream::comment_issue(repo, number, &format!("Dropped: {}", because.trim()))?` and `upstream::close_issue(repo, number, "not planned")?`.
     - Print confirmation naming `#{number}`.
- **The trap**: Always validate `docs/ideas/` exists and `ideas::locate` resolves BEFORE fetching or mutating, and ALWAYS commit locally BEFORE calling `comment_issue`/`close_issue` so a failed local commit never closes a GitHub issue.
- Add integration test `upstream_pull_lists_and_triages_issues_to_idea_fold_and_drop` in `tests/upstream_hardening.rs` proving:
  1. `factory upstream pull` and `factory upstream pull --json` list open issues from `--repo github.com/emilianotisato/factory`.
  2. `factory upstream pull --idea 42` creates `docs/ideas/YYYY-MM-DD-*.md` with `origin: "<issue url>"`, commits it (`git status --porcelain` clean), comments via `--body-file -` (stdin logged, not on argv), and closes `#42` (`--reason completed`).
  3. `factory upstream pull --fold 43 <id>` appends issue `#43`'s title and body into the existing idea file, commits it, comments via `--body-file -`, and closes `#43` (`--reason completed`).
  4. `factory upstream pull --drop 44 --because "duplicate"` comments `Dropped: duplicate` via `--body-file -` and closes `#44` (`--reason "not planned"`) with no git commit.
  5. Refuses inside a run (`FACTORY_BOARD` set) without calling `gh`, and refuses `--idea` when `docs/ideas/` is missing without calling `gh`.

**Acceptance:** `cargo test --test upstream_hardening upstream_pull_lists_and_triages_issues_to_idea_fold_and_drop` passes, proving all 5 scenarios (fails if any pull mode, stdin comment, local-commit-first ordering, `docs/ideas/` check, or `BOARD_ENV` guard is broken).

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
**Source**: §8 QA plan FR-3 row: "run `upstream_pull_lists_and_triages_issues_to_idea_fold_and_drop`".

At Checkpoint 2, run `cargo test --test upstream_hardening upstream_pull_lists_and_triages_issues_to_idea_fold_and_drop` alongside the full gate and record the transcript in **Progress**.

**Acceptance:** Checkpoint transcript in **Progress** confirms `upstream_pull_lists_and_triages_issues_to_idea_fold_and_drop` passes (would fail if any pull list/json/idea/fold/drop behavior or guardrail were broken).

**Code-review fixes (2026-11-27).**
- `pull --idea` / `--fold` refuse when `commit` stages nothing instead of closing GitHub anyway.
- `--drop` test compares `HEAD` before/after; pull triage tests assert stdin comments use `--body-file -`.
