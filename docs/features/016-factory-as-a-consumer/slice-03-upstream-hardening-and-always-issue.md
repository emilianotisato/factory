# Slice 03 — Upstream hardening and always-issue post

**Source**: [`prd.md`](prd.md) → §4 (FR-2). **Depends on**: Slice 02 (so FR-4 home-path screening is live when `upstream check` tests run and when `skills/harvest/SKILL.md:61` drops the outdated no-trailing-slash caveat)
**Created**: 2026-11-27

**Progress**: `ci::gh_stdin` + `exec_stdin` (BrokenPipe-tolerant); `upstream::REPO` host-qualified; all-remotes `names`, `name_variants`, `draft_hash`; `upstream check/post` with `--allow` and required `--hash`; always `open_issue` via stdin; `tests/upstream_hardening.rs`; harvest skill + `0.19.0` bump. Inner loop: 5 runs, 3 red (~10 min) — bundle.lock not regenerated before first full `cargo test`; clippy collapsible-if / duplicate `#[test]`; hardening borrow lifetimes. Deliberate red: dropped `hash:` print → `post_without_a_source_checkout` panicked on missing hash line. cp-2/FR-2/upstream.txt · cp-2/FR-2/upstream-hardening.txt

**Slice**: Hardens `factory upstream` for consumer use and makes `factory upstream post` always open a GitHub issue on `github.com/emilianotisato/factory`: adds `ci::gh_stdin` (tolerating `BrokenPipe`), host-qualifies `REPO`, removes the `[source]` local-idea shortcut, scans all configured git remotes, expands name-side spelling variants plus `--allow <word>`, ties `check` and `post` via a 12-hex SHA256 `--hash`, updates `skills/harvest/SKILL.md`, and bumps version `0.18.0` → `0.19.0` across crate, bundle lock, and cockpit mod. Leaves `factory upstream pull` to Slice 04.

**Highest-risk**: `testguard` boundaries in `tests/upstream.rs` (only 3 test functions are authorized; shared helpers and untouched tests must stay untouched), `ci::gh_stdin` `BrokenPipe` handling against early-exiting stubs, and keeping the `0.18.0` → `0.19.0` bump synchronized across all 6 versioned files.

## Current code / environment

- **`src/ci.rs:66-85`**: `exec`, `gh`, and `git` use `Command::output()` with no stdin piping. Needs `pub fn gh_stdin(repo: &Path, args: &[&str], stdin_bytes: &[u8]) -> Result<Output>`.
- **`src/upstream.rs:10`**: `pub const REPO: &str = "emilianotisato/factory";` — omits `github.com/`, so `GH_HOST` can redirect to a client's GitHub Enterprise.
- **`src/upstream.rs:48-63`**: `names(repo)` runs only `preflight::git(repo, &["remote", "get-url", "origin"])`, missing remotes named `upstream`, `github`, etc.
- **`src/upstream.rs:131-160`**: `holds_word` checks exact whole-word match only. Unit tests at `src/upstream.rs:245-257` (`the_repo_name_hits_as_a_whole_word_in_any_case`, `a_word_that_only_contains_the_name_does_not_hit`) assert exact hit lists (e.g., `"one\nthe Acme-Api broke"` with `&["acme-api"]` yields `[(2, "this repo's name: acme-api")]` — single hit — and `"acme_api"` with `&["acme"]` yields empty!).
- **`src/metrics.rs:160`**: `pub fn sha256_hex(bytes: &[u8]) -> String` already exists for computing `draft_hash`.
- **`src/cli/upstream.rs:15-161`**: `UpstreamVerb::Check { draft }` and `UpstreamVerb::Post { draft }` have no `--allow` or `--hash`. `post()` (`:81-92`) branches on `loaded.config.source` to call `add_idea` (`:94-129`); `open_issue` (`:131-161`) passes `--body &draft.body` on argv.
- **`tests/upstream.rs:1-203`**: Authorized tests in `prd.md` frontmatter are ONLY:
  1. `tests/upstream.rs::a_failed_issue_search_is_one_line_and_not_a_hit` (`:118`)
  2. `tests/upstream.rs::post_without_a_source_checkout_opens_an_issue` (`:132`)
  3. `tests/upstream.rs::post_with_a_source_checkout_commits_one_idea_and_opens_no_issue` (`:152`)
  **Do NOT edit the shared `upstream()` helper at `:28` or any other test** (`post_refuses_a_draft_with_a_hit` at `:174` and `inside_a_run_check_and_post_refuse_without_calling_gh` at `:184` stay green untouched because `post` checks `BOARD_ENV` and leak hits BEFORE requiring `--hash`). Add a new helper (e.g., `upstream_with_args`) for passing `--hash` inside the authorized test functions!
- **`skills/harvest/SKILL.md:38-40, :61`** and **version `0.18.0` → `0.19.0`**: Because `v0.18.0` is tagged, editing `skills/harvest/SKILL.md` requires bumping `0.18.0` → `0.19.0` in `Cargo.toml`, `Cargo.lock`, `skills/bundle.lock` (`FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle`), `mods/cockpit/.claude-plugin/plugin.json:3`, `mods/cockpit/hooks/register.tsx:5`, and `mods/cockpit/tests/cockpit.test.tsx:5` (authorized).
- **Deliberate non-change**: `/harvest` inside the `factory` repo checkout itself still writes directly to `docs/ideas/` via `factory docs idea add` *(prd §10 Non-goals decided against forcing local factory harvest through GitHub issues. Keep `skills/harvest/SKILL.md:34` intact.)*

## Tasks

### 1. Add `ci::gh_stdin` tolerating `BrokenPipe` — high — blocked by: —
**Source**: §4 FR-2: "add `pub fn gh_stdin(repo: &Path, args: &[&str], stdin_bytes: &[u8]) -> Result<Output>` piping `stdin_bytes` to `gh`'s stdin, ignoring `ErrorKind::BrokenPipe` on the write (so an early-exiting `gh` or test stub that does not read stdin never flakes before `wait_with_output()` returns the real exit status and stderr!)."

`src/ci.rs`.
- Spawn `Command::new("gh").args(args).current_dir(repo).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()`.
- Take `child.stdin.take()`, call `stdin.write_all(stdin_bytes)`: if `Err(e)` where `e.kind() == ErrorKind::BrokenPipe`, ignore it; propagate other I/O errors; drop `stdin` so the child sees EOF.
- Call `child.wait_with_output()` and return `Output { stdout, stderr, code }`.
- **The trap**: Never fail on `BrokenPipe` during `write_all` — `tests/stubs/gh` exits without reading stdin, and a real `gh` auth failure exits immediately too. Add a unit test in `src/ci.rs` exercising `gh_stdin` (or a private helper `exec_stdin`) against an early-exiting command (e.g. `true` / `false`) with a large payload (`> 128 KiB`) to prove `BrokenPipe` is ignored and exit code is returned cleanly.

**Acceptance:** Unit test in `src/ci.rs` passes a `> 128 KiB` stdin payload to an early-exiting child and asserts `Ok(Output)` with the child's exit code rather than a broken-pipe error (fails if `BrokenPipe` ignoring is removed).

### 2. Harden `src/upstream.rs` (`REPO`, all remotes, name-side variants, `draft_hash`) — high — blocked by: —
**Source**: §4 FR-2: "change `pub const REPO: &str = \"github.com/emilianotisato/factory\";`... Scan ALL git remotes in `upstream::names`... Expand on the name side (never fold the input line) and emit at most one hit per `(line, name)`... For a repo name/owner containing `-`, `_`, or space, check: the exact name, separator-swapped variants (`-`, `_`, space), the concatenated form (`acmeapi` matching `AcmeApi` case-insensitively as a whole word), and each split part of length `>= 4`... Add `pub fn draft_hash(text: &str) -> String`: first 12 hex chars of `metrics::sha256_hex(text.as_bytes())`."

`src/upstream.rs`.
- Change `pub const REPO: &str = "github.com/emilianotisato/factory";`.
- Update `names(repo: &Path) -> Vec<String>`: run `preflight::git(repo, &["remote"])` to list all configured remotes, and for each non-empty remote name run `preflight::git(repo, &["remote", "get-url", remote])`, pushing `(owner, name)` from `owner_and_name(url.trim())`. Keep directory name, sort, dedup.
- Add `pub fn draft_hash(text: &str) -> String` returning `&metrics::sha256_hex(text.as_bytes())[..12]` as `String`.
- Add helper `name_variants(name: &str) -> Vec<String>` (name-side expansion only — never fold the input line!):
  - Always include `name` itself.
  - If `name` contains any of `'-'`, `'_'`, `' '`: split on `['-', '_', ' ']` into non-empty parts. If `parts.len() >= 2`:
    - Push separator-swapped joins: `parts.join("-")`, `parts.join("_")`, `parts.join(" ")`.
    - Push concatenated form: `parts.concat()`.
    - Push each part with `part.chars().count() >= 4`.
  - Deduplicate variants (case-insensitively).
- In `hits(text: &str, names: &[String], homes: &[String]) -> Vec<Hit>`:
  - For each `name` in `names`, check `name_variants(name).iter().any(|v| holds_word(line, v))` — emitting **at most one** `Hit { line: i + 1, what: format!("this repo's name: {name}") }` per `(line, name)`.
- **The trap**: Never fold the input line and never split a single-word name; that keeps existing `src/upstream.rs:245-257` unit tests (`"acme_api"` with `&["acme"]` → empty, `"one\nthe Acme-Api broke"` with `&["acme-api"]` → one hit) passing verbatim!
- Add unit tests in `src/upstream.rs` for multi-remote scanning (`upstream`/`github` without `origin`), spelling variants (`AcmeApi`, `acme_api`, `acme api`, `acme` hitting for `acme-api`, while `api` does not hit as a split part), and `draft_hash` returning 12 lowercase hex chars.

**Acceptance:** Unit tests in `src/upstream.rs` prove all remotes are scanned, `acme-api` catches `AcmeApi`/`acme_api`/`acme api`/`acme` with one hit per line while skipping `< 4`-char parts like `api`, and `draft_hash` produces a deterministic 12-hex string (fails if remote loop, variant expansion, or `draft_hash` is removed).

### 3. Update `src/cli/upstream.rs` (always issue via stdin, `--allow`, required `--hash`) — high — blocked by: 1, 2
**Source**: §4 FR-2: "Remove `[source]` shortcut from `cli::upstream::post`... delete `add_idea` and the `loaded.config.source` branch... Use `ci::gh_stdin(repo, &[\"issue\", \"create\", \"--repo\", REPO, \"--title\", &draft.title, \"--body-file\", \"-\"], draft.body.as_bytes())` in `open_issue`... Add `#[arg(long = \"allow\", value_name = \"WORD\")] pub allow: Vec<String>` to both `UpstreamVerb::Check` and `UpstreamVerb::Post`... `factory upstream check <draft>` prints `hash: <12-hex>` when the draft has zero hits... `factory upstream post <draft> --hash <12-hex>`: Execution order: (1) refuse inside a run (`BOARD_ENV`), (2) screen for leak hits (exiting `1` if any hit), (3) require `--hash` to be present AND equal to `draft_hash(&text)`."

`src/cli/upstream.rs`, `tests/upstream.rs`, `tests/upstream_hardening.rs` (new).
- On `UpstreamVerb::Check`: add `#[arg(long = "allow", value_name = "WORD")] pub allow: Vec<String>`.
- On `UpstreamVerb::Post`: add `#[arg(long = "allow", value_name = "WORD")] pub allow: Vec<String>` and `#[arg(long)] pub hash: Option<String>`.
- In `screen(repo, file, allow, out)` → return `(Draft, String, bool)` (draft, raw text, hit):
  - Filter `upstream::names(repo)` by excluding any name whose lowercase matches any `allow` entry's lowercase (`allow` ONLY filters repo names — NEVER home-path or email hits!).
  - Print hits as before.
- In `check(...)`:
  - Run `screen(...)` and `upstream::search(...)` as before.
  - When `!hit`, print `hash: <12-hex>` (from `upstream::draft_hash(&text)`).
- In `post(...)`:
  1. `BOARD_ENV` refusal already runs first in `upstream()`.
  2. Run `screen(...)`; if `hit`, print `not posted: rewrite the lines above in general terms` and return `Ok(1)`.
  3. Validate `hash`: compute `expected = upstream::draft_hash(&text)`. Require `hash.as_deref() == Some(expected.as_str())`; if missing or mismatched, print an error naming `factory upstream check <file>` (e.g. `not posted: --hash missing or does not match draft — run \`factory upstream check <file>\` and pass --hash <hash>`) and return `Ok(1)` without calling `gh`.
  4. If `!allow.is_empty()`, print `allowed: <comma-separated words>` to `out`.
  5. Delete `add_idea`, `ORIGIN`, and the `config::load` / `source` branch; always call `open_issue(repo, &draft, out)`.
- In `open_issue`: call `ci::gh_stdin(repo, &["issue", "create", "--repo", REPO, "--title", &draft.title, "--body-file", "-"], draft.body.as_bytes())?`.
- Update the 3 authorized tests in `tests/upstream.rs` (`a_failed_issue_search_is_one_line_and_not_a_hit` expecting `github.com/emilianotisato/factory`, `post_without_a_source_checkout_opens_an_issue` passing `--hash` via a NEW helper and checking `--repo github.com/emilianotisato/factory` + `--body-file -`, and `post_with_a_source_checkout_commits_one_idea_and_opens_no_issue` updated to assert that even with `[source]` configured, `post --hash <h>` opens a GitHub issue and makes NO local commit in `[source]`). **Do NOT edit shared `upstream()` at `:28` or any unauthorized test!**
- Add `tests/upstream_hardening.rs` with `upstream_hardening_host_stdin_remotes_variants_allow_and_hash` using a fresh tempdir executable `gh` stub that logs argv AND stdin to separate files, proving:
  1. `gh` receives `--repo github.com/emilianotisato/factory` and `--body-file -`, with the draft body on stdin and NOT in argv.
  2. `post` opens a GitHub issue even when `[source]` is set.
  3. Remotes named `upstream` / `github` (with no `origin`) are screened.
  4. `acme-api` catches `AcmeApi`, `acme_api`, `acme api`, and `acme` (one hit per line), while `--allow api` lets a repo literally named `api` pass (echoing `allowed: api`) and still blocks email and home-path hits.
  5. `check` prints `hash: <12-hex>`; `post` without `--hash` or with a stale hash after editing exits `1` with zero `gh` calls; `post --hash <12-hex>` exits `0`.

**Acceptance:** `cargo test --test upstream` and `cargo test --test upstream_hardening` pass (fails if host prefix, stdin piping, `[source]` removal, multi-remote screening, variant/allow handling, or `--hash` enforcement is removed).

### 4. Update `skills/harvest/SKILL.md` and bump version `0.18.0` → `0.19.0` — high — blocked by: 3
**Source**: §4 FR-2 & §2 Blast radius: "Update `skills/harvest/SKILL.md:38-40` to pass `--hash <hash>` from `factory upstream check` into `factory upstream post <draft> --hash <hash>` and `:61` to drop the outdated \"misses homes with no `/`\" sentence, and bump crate/bundle/cockpit version `0.18.0` → `0.19.0` (`Cargo.toml`, `Cargo.lock`, `skills/bundle.lock`, `mods/cockpit/.claude-plugin/plugin.json`, `mods/cockpit/hooks/register.tsx`, `mods/cockpit/tests/cockpit.test.tsx`)."

`skills/harvest/SKILL.md`, `Cargo.toml`, `Cargo.lock`, `skills/bundle.lock`, `mods/cockpit/.claude-plugin/plugin.json`, `mods/cockpit/hooks/register.tsx`, `mods/cockpit/tests/cockpit.test.tsx`, `tests/skills_bundle.rs`.
- Edit `skills/harvest/SKILL.md:38-40`: note that a clean `factory upstream check <draft>` prints `hash: <hash>`, and step 5 runs `factory upstream post <draft> --hash <hash>` only on the owner's yes.
- Edit `skills/harvest/SKILL.md:61`: remove the stale last sentence `Neither the check nor --done sees a home with no / after the name (find /home/<name> …): grep the kept files for one.` (keep the rest of the bullet explaining `W-HOME-PATH` and `--done`).
- Bump `0.18.0` → `0.19.0` in:
  - `Cargo.toml` (`version = "0.19.0"`)
  - `Cargo.lock` (package `factory` version)
  - `mods/cockpit/.claude-plugin/plugin.json` (`"version": "0.19.0"`)
  - `mods/cockpit/hooks/register.tsx` (`const MOD_VERSION = '0.19.0'`)
  - `mods/cockpit/tests/cockpit.test.tsx` (`const VERSION = '0.19.0'`)
- Run `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` to regenerate `skills/bundle.lock` at `0.19.0`.
- Add a skill-text test in `tests/skills_bundle.rs` using `assert_unique_passage` to assert that `skills/harvest/SKILL.md` contains the exact unique passage `factory upstream post <draft> --hash <hash>` once and does NOT contain `Neither the check nor \`--done\` sees a home`.

**Acceptance:** `cargo test --test skills_bundle` and `claude plugin validate mods/cockpit && claude plugin test mods/cockpit` pass at `0.19.0`, with the skill-text test asserting `factory upstream post <draft> --hash <hash>` occurs exactly once in `skills/harvest/SKILL.md` and the stale no-trailing-slash sentence is absent (fails if either `SKILL.md` edit is reverted or any version file stays at `0.18.0`).

### 5. Verify — medium — blocked by: 1, 2, 3, 4
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 6. Prove — at the checkpoint — blocked by: 5
**Source**: §8 QA plan FR-2 row: "run `cargo test --test upstream` and `upstream_hardening_host_stdin_remotes_variants_allow_and_hash`".

At Checkpoint 2, run `cargo test --test upstream`, `cargo test --test upstream_hardening upstream_hardening_host_stdin_remotes_variants_allow_and_hash`, `cargo test --test skills_bundle`, and `claude plugin test mods/cockpit`, recording the transcript in **Progress**.

**Acceptance:** Checkpoint transcript in **Progress** confirms all FR-2 integration, bundle, and cockpit tests pass (would fail if any hardening invariant or version sync were broken).

**Code-review fixes (2026-11-27).**
- `post` no longer prints the expected `--hash` in its error (forces a real `check` run).
- Hardened `tests/upstream_hardening.rs` and `tests/upstream.rs` to assert `--body-file -`, zero `gh` on stale hash, and `github` remote screening without `--allow`.
