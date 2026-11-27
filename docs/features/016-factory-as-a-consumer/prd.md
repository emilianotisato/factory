---
number: 016
slug: factory-as-a-consumer
title: "Factory as a consumer"
type: feature
created: 2026-11-05
author: Emiliano Tisato
authorized_tests: ["mods/cockpit/tests/cockpit.test.tsx", "tests/upgrade.rs::no_receipt_and_no_source_prints_the_installer", "tests/upgrade.rs::a_receipt_for_another_prefix_is_from_source", "tests/upstream.rs::a_failed_issue_search_is_one_line_and_not_a_hit", "tests/upstream.rs::post_without_a_source_checkout_opens_an_issue", "tests/upstream.rs::post_with_a_source_checkout_commits_one_idea_and_opens_no_issue"]
summary: |
  Lets the owner run `factory` as a normal consumer: adds a rolling `edge` prerelease workflow on `main` pushes (`factory upgrade --edge`), fixes receipt-vs-source detection and allows `factory upgrade` to install from GitHub Releases even without an existing receipt (stripping `/bin` for cargo-home layout, retiring the `cargo install` rule from `CLAUDE.md`/`AGENTS.md`); makes `factory upstream post` always open a GitHub issue (removing the `[source]` local shortcut) and adds `factory upstream pull` (list/`--json` + `idea`/`fold`/`drop` triage closing issues with comments); hardens `factory upstream` (`github.com/` host, `--body-file -` stdin, all remotes, spelling variants + `--allow`, required `--hash` from `check`, and `skills/harvest/SKILL.md` + `0.19.0` bump); and extends `paths.rs` home-path detection/rewriting to all shapes including no trailing slash.
---

# PRD-016 — Factory as a consumer

> **Interviewed 2026-11-27** (continuing the 2026-11-05 draft; every `file:line`, `axoupdater` 0.10.2 API, live `v0.18.0` `factory-installer.sh`, and workflow rule below was verified against `main` at `e446f6d`). Revised 2026-11-27 after cross-vendor adversarial pass (`claude:opus`, log `20261127-165828`).

## 1. Why now

Today the owner still dogfoods `factory` through developer shortcuts instead of the consumer road:

- **Source installs and broken receipt detection.** `CLAUDE.md:6` and `AGENTS.md:6` mandate `cargo install --path . --locked` after every merge; `dist-workspace.toml:15` sets `install-path = "CARGO_HOME"`, so `axoupdater::check_receipt_is_for_this_executable` (`src/cli/upgrade.rs:22`) confuses a `cargo install` build in `~/.cargo/bin/factory` with an older release install; `factory upgrade` (`src/cli/upgrade.rs:58-66`) refuses to upgrade whenever no receipt exists (even with no `[source]` configured); and there is no rolling `edge` prerelease build on `main` pushes for testing between tagged releases.
- **`factory upstream post` bypasses GitHub when `[source]` is set, and there is no triage verb for incoming issues.** `cli::upstream::post` (`src/cli/upstream.rs:88-91`) commits straight into `[source]` instead of opening a GitHub issue, and there is no `factory upstream pull` verb to list and triage open issues into `docs/ideas/` and close them with a pointer comment.
- **`factory upstream` has security and usability gaps now that GitHub issues are the only road.** `upstream::REPO` (`src/upstream.rs:10`) omits `github.com/` (so `GH_HOST` can misroute issues to a client's GitHub Enterprise); `open_issue` (`src/cli/upstream.rs:141-142`) puts the draft body on argv (`--body <text>`, visible in `ps` and subject to the 128 KiB argv cap); `upstream::names` (`src/upstream.rs:53`) reads only `origin` (missing `upstream`/`github` remotes); `holds_word` misses spelling variants (`AcmeApi`, `acme_api`, `acme api`, hyphenated parts) and has no `--allow <word>` override for short/common repo names (`api`, `web`, `app`); and `check` and `post` are not tied by a draft content hash.
- **Home paths without a trailing slash slip past `W-HOME-PATH`, archive rewrites, and `upstream check`.** `paths::HOME_PATH` (`src/paths.rs:95-103`) requires a trailing `/` (`/(?:home|Users)/[A-Za-z0-9._-]+/`), missing bare `/home/<name>` at end-of-word/end-of-line as well as WSL `/mnt/c/Users/<name>`, Windows `C:\Users\<name>`, `/root/`, case-insensitive `/users/<name>`, and JSON-escaped `\/home\/<name>`.

**What this leaves alone.** Cross-repo ideas scanner (`2026-10-12-cross-repo-ideas-scanner-factory-ideas-json-plus-a.md`, explicitly out of scope per 2026-11-05 triage), changing `dist-workspace.toml` `install-path` away from `CARGO_HOME` (would break existing users' `PATH`), and changing `/harvest` inside the `factory` repo itself (filing a factory lesson locally inside the factory checkout stays direct via `factory docs idea add`).

## 2. Blast radius

- **`mods/cockpit/tests/cockpit.test.tsx` (authorized) and `0.18.0` → `0.19.0` version bump.** Because `v0.18.0` is already tagged on `main`, updating `skills/harvest/SKILL.md:38-40` (for `--hash`) and `:61` (removing the stale "misses homes with no `/`" note) requires bumping `0.18.0` → `0.19.0` across `Cargo.toml`, `Cargo.lock`, `skills/bundle.lock` (via `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle`), `mods/cockpit/.claude-plugin/plugin.json`, `mods/cockpit/hooks/register.tsx`, and `mods/cockpit/tests/cockpit.test.tsx`.
- **`tests/upgrade.rs::no_receipt_and_no_source_prints_the_installer` and `tests/upgrade.rs::a_receipt_for_another_prefix_is_from_source` (authorized).** Under FR-1, when `[source]` is unset in `config.toml`, `factory upgrade --check` queries GitHub Releases directly even with no receipt (or a mismatched-prefix/mismatched-version receipt) instead of printing `installed from source — curl ...`. Both tests update to pass a local stub server with `--check` (never invoking `run_sync()` against real `github.com`!), while `no_receipt_prints_the_source_line_from_config` (`:123`, where `[source]` IS set) stays green untouched. **Do NOT edit shared helpers (`release_server`, `Env::receipt`) outside authorized test functions** — add new helpers instead so `testguard` stays clean.
- **`tests/upstream.rs::a_failed_issue_search_is_one_line_and_not_a_hit`, `post_without_a_source_checkout_opens_an_issue`, and `post_with_a_source_checkout_commits_one_idea_and_opens_no_issue` (authorized).** Under FR-2, `upstream` uses `--repo github.com/emilianotisato/factory`, passes body via `--body-file -` on stdin, requires `--hash <hash>` from `check`, and ALWAYS opens a GitHub issue even when `[source]` is set. (`post_refuses_a_draft_with_a_hit` at `:174` and `inside_a_run_check_and_post_refuse_without_calling_gh` at `:184` stay green untouched because `post` checks `BOARD_ENV` and leak hits before requiring `--hash`.) **Do NOT edit the shared `upstream()` helper at `tests/upstream.rs:28`** — add a new helper for extra args so `testguard` stays clean.
- **`tests/release.rs:62-72` (`the_readme_offers_the_installer_before_cargo_install`).** Asserts `README.md` contains both `releases/latest/download/factory-installer.sh` and `cargo install --path .` (with installer first). Keep a contributor-build mention of `cargo install --path .` in `README.md` while replacing the stale receipt-workaround paragraph (`README.md:107`) and retiring the `cargo install` rule from `CLAUDE.md:6` and `AGENTS.md:6`.
- **`src/paths.rs:209` unit test.** Currently asserts bare `/home/<name>` without trailing slash is `None`. Under FR-4, bare `/home/<name>` followed by space IS a hit (`Some(0)`), while bare `/home/` and `/Users` remain `None`; update that unit test in `src/paths.rs`.

## 3. FR-1 — Smart `factory upgrade` (receipt version check, receipt-less upgrade, `--edge`) and rolling `edge` workflow

**Today.**
- `.github/workflows/release.yml` runs `cargo-dist` on `v*` version tags only (`on: push: tags: '**[0-9]+.[0-9]+.[0-9]+*'`).
- `src/cli/upgrade.rs:19-30` (`updater`/`from_release`) trusts `axoupdater::check_receipt_is_for_this_executable()`, which only checks parent directory (`~/.cargo`), so a `cargo install` binary of another version is mistaken for a release install.
- `src/cli/upgrade.rs:58-66` prints `installed from source — curl ... | sh` and refuses to upgrade whenever `updater()` returns `None`, even when `[source]` is unset.
- `UpgradeArgs` (`:12-17`) has only `--check`, with no `--edge` prerelease opt-in.
- `CLAUDE.md:6` and `AGENTS.md:6` instruct running `cargo install --path . --locked` after every merge; `README.md:107` documents the receipt confusion workaround.

**Decided 2026-11-27.**

- **Rolling `edge` prerelease workflow (`.github/workflows/edge.yml`):**
  - Triggers on `push` to `main` (with `paths-ignore: ['docs/**', 'README.md', 'CLAUDE.md', 'AGENTS.md']` so docs/harvest commits do not burn runner minutes) and `workflow_dispatch`, with `permissions: { contents: write }` and `concurrency: { group: edge-release, cancel-in-progress: true }`.
  - Runs directly inside `edge.yml` (does NOT rely on pushing a tag with `GITHUB_TOKEN`, since GitHub Actions blocks `GITHUB_TOKEN` pushes from triggering `release.yml`).
  - `plan` job uses `actions/checkout@v6` with `fetch-depth: 0` (so all git tags are visible!), reads `version` from `Cargo.toml` (`X.Y.Z`), checks `git rev-parse -q --verify "refs/tags/v${VER}"` (if that tag already exists, increments patch to `X.Y.(Z+1)` so the prerelease always sorts strictly above the latest stable tag!), sets `EDGE_VER="${BASE}-edge.${GITHUB_RUN_NUMBER}"`, and exports `edge_ver` and `edge_tag="v${EDGE_VER}"` as job outputs.
  - Every build job (`build-local-artifacts` across the 4 Unix targets and `build-global-artifacts`) checks out `main` and runs the same version-patch step updating `Cargo.toml` and `Cargo.lock` in the CI workspace (never committed) to `EDGE_VER` before invoking `dist build --tag="${EDGE_TAG}" ...`.
  - Publish job runs `gh release create "${EDGE_TAG}" --prerelease --target "${GITHUB_SHA}" --title "factory ${EDGE_TAG}" --notes "Rolling edge build from ${GITHUB_SHA}." artifacts/*`, then deletes older `*-edge.*` prereleases (and their tags) so only the newest rolling edge prerelease remains.
- **Fix receipt version check and receipt-less upgrade in `src/cli/upgrade.rs`:**
  - Read `factory-receipt.json` from the same config search order `axoupdater` uses (`AXOUPDATER_CONFIG_WORKING_DIR`, `AXOUPDATER_CONFIG_PATH`, `XDG_CONFIG_HOME/factory`, `~/.config/factory`): require `receipt.version == env!("CARGO_PKG_VERSION")` alongside `u.check_receipt_is_for_this_executable().unwrap_or(false)` inside `from_release()` / `matching_receipt()`. A `cargo install` binary whose version differs from the receipt is no longer misclassified by `from_release()` or `ahead_hint()`.
  - Add pure helper `pub fn install_prefix_from_exe(exe: &Path) -> PathBuf` (caught 2026-11-27 in adversarial pass & verified against live `v0.18.0` `factory-installer.sh:595-600`): take `exe.parent()` and strip a trailing `"bin"` component when present (so `~/.cargo/bin/factory` → `~/.cargo`), because `FACTORY_INSTALL_DIR` / `CARGO_DIST_FORCE_INSTALL_DIR` uses `_install_layout="cargo-home"` which appends `/bin` (passing `~/.cargo/bin` would install into `~/.cargo/bin/bin/factory` and break future receipt matching!).
  - Add `#[arg(long)] pub edge: bool` to `UpgradeArgs` (`src/cli/upgrade.rs:12-17`).
  - In `upgrade(args, out)` (`:58-100`):
    - Load `config::load(&paths::config_dir())?.config`.
    - If `cfg.source.is_some()` AND `!from_release()` AND `!args.edge` (explicit developer source opt-in): keep printing `installed from source — cargo install --path <dir>` and return `Ok(0)`.
    - Otherwise (consumer path — whether a matching receipt exists or NOT):
      - If a matching receipt loaded, use that `AxoUpdater`; else build `AxoUpdater::new_for("factory")`, configure `ReleaseSource { release_type: ReleaseSourceType::GitHub, owner: "emilianotisato".into(), name: "factory".into(), app_name: "factory".into() }`, and call `u.set_install_dir(install_prefix_from_exe(&paths::self_exe()?))`.
      - Set `current_version` to `env!("CARGO_PKG_VERSION")`.
      - Set `configure_version_specifier(if args.edge { UpdateRequest::LatestMaybePrerelease } else { UpdateRequest::Latest })`.
      - With `--check`: query `query_new_version()`, print `{current} → {v}` when `v > current` else `already current`.
      - Without `--check`: run `u.run_sync()` and print `{old} → {new}` or `already current`.
- **Update repo docs (`CLAUDE.md`, `AGENTS.md`, `README.md`):**
  - Replace the `cargo install --path . --locked` rule in `CLAUDE.md:6` and `AGENTS.md:6` with consumer upgrade guidance (`factory upgrade` / `factory upgrade --edge` once a release builds; `cargo install --path . --locked` only when testing local unmerged builds).
  - Replace the receipt-confusion workaround at `README.md:107` with how `factory upgrade` and `factory upgrade --edge` work, keeping a mention of `cargo install --path .` after the installer URL so `tests/release.rs:62-72` stays green.

**Accepted cost.** A `cargo install` binary of the *exact same* version as an existing `~/.config/factory/factory-receipt.json` remains indistinguishable from that release install (harmless — upgrading only replaces with a strictly newer version). Live execution of `.github/workflows/edge.yml` is proven on the first post-merge push to `main`.

**Acceptance.** Integration & unit tests (`tests/upgrade.rs` + `tests/release.rs` + `src/cli/upgrade.rs`) prove: (1) `install_prefix_from_exe` maps `~/.cargo/bin/factory` → `~/.cargo`, `/usr/local/bin/factory` → `/usr/local`, and `/opt/factory/factory` → `/opt/factory`; (2) a receipt whose `version` does not match `CARGO_PKG_VERSION` makes `from_release()` return `false` and `ahead_hint()` respect `[source]` when set; (3) with NO receipt and NO `[source]`, `factory upgrade --check` against a local stub release server prints `{V} → 999.0.0` (and `already current` on `v{V}`), while `[source]` still prints `installed from source — cargo install --path /src/factory`; (4) `factory upgrade --check --edge` against a local stub server handling `/api/v3/repos/emilianotisato/factory/releases` (matching by path prefix) prints `{V} → 999.0.0-edge.42` while plain `--check` ignores prereleases; (5) `.github/workflows/edge.yml` exists, checks out with `fetch-depth: 0`, triggers on `push` to `main` (ignoring `docs/**`), patches `Cargo.toml`/`Cargo.lock` in each build job, creates a `--prerelease`, and prunes older `*-edge.*` prereleases.

## 4. FR-2 — Always open GitHub issues in `factory upstream post` and harden `factory upstream`

**Today.**
- `src/upstream.rs:10`: `pub const REPO: &str = "emilianotisato/factory";` (bare form — resolved against `GH_HOST`, which can point at a client's GitHub Enterprise).
- `src/upstream.rs:48-63`: `names(repo)` only runs `git remote get-url origin`, missing `upstream` / `github` / other remotes.
- `src/upstream.rs:131-154`: `holds_word` matches exact name only, missing spelling variants (`AcmeApi`, `acme_api`, `acme api`, hyphenated tokens) and offering no `--allow <word>` override for short/common repo names (`api`, `web`, `app`).
- `src/cli/upstream.rs:88-91`: `post` checks `loaded.config.source` and calls `add_idea(...)` instead of `open_issue(...)` whenever `[source]` is set.
- `src/cli/upstream.rs:131-145`: `open_issue` passes `--body <text>` on argv (visible in `ps`, capped by OS `ARG_MAX`) instead of `--body-file -` via stdin.
- `check` and `post` are not tied by a draft content hash.

**Decided 2026-11-27.**

- **Host-qualified repo (`src/upstream.rs:10`):** change `pub const REPO: &str = "github.com/emilianotisato/factory";` everywhere `gh --repo` is called (search, create, pull/list, view, comment, close) so `GH_HOST` can never redirect a draft to a private GitHub Enterprise server.
- **Stdin support for `gh` (`src/ci.rs:66-85`):** add `pub fn gh_stdin(repo: &Path, args: &[&str], stdin_bytes: &[u8]) -> Result<Output>` piping `stdin_bytes` to `gh`'s stdin, ignoring `ErrorKind::BrokenPipe` on the write (so an early-exiting `gh` or test stub that does not read stdin never flakes before `wait_with_output()` returns the real exit status and stderr!). Use `ci::gh_stdin(repo, &["issue", "create", "--repo", REPO, "--title", &draft.title, "--body-file", "-"], draft.body.as_bytes())` in `open_issue` (and for `issue comment --body-file -` in FR-3).
- **Remove `[source]` shortcut from `cli::upstream::post` (`src/cli/upstream.rs:81-129`):** delete `add_idea` and the `loaded.config.source` branch — `factory upstream post` ALWAYS opens a GitHub issue on `github.com/emilianotisato/factory`.
- **Scan ALL git remotes in `upstream::names` (`src/upstream.rs:48-63`):** run `git remote` to list every configured remote name and call `git remote get-url <remote>` for each, extracting `(owner, name)` from every remote (not just `origin`).
- **Spelling variants and `--allow <word>` (`src/upstream.rs`, `src/cli/upstream.rs`):**
  - Expand **on the name side (never fold the input line)** and emit **at most one hit per `(line, name)`** (stopping on the first variant that matches via `holds_word`, so existing unit tests `the_repo_name_hits_as_a_whole_word_in_any_case` and `a_word_that_only_contains_the_name_does_not_hit` stay exact!):
    - For a repo name/owner containing `-`, `_`, or space, check: the exact name, separator-swapped variants (`-`, `_`, space), the concatenated form (`acmeapi` matching `AcmeApi` case-insensitively as a whole word), and each split part of length `>= 4` (so `acme-api` checks `acme-api`, `acme_api`, `acme api`, `acmeapi`, and `acme`, while skipping tiny 2–3 char parts like `api`, `web`, `app`).
  - Add `#[arg(long = "allow", value_name = "WORD")] pub allow: Vec<String>` to both `UpstreamVerb::Check` and `UpstreamVerb::Post`: allowed words (case-insensitive) are skipped from repo-name hits (never from home-path or email hits!), and `post` echoes `allowed: <words>` to stdout so the override is always visible.
- **Tie `check` and `post` by draft SHA256 hash (`src/upstream.rs`, `src/cli/upstream.rs`, `skills/harvest/SKILL.md`):**
  - Add `pub fn draft_hash(text: &str) -> String`: first 12 hex chars of `metrics::sha256_hex(text.as_bytes())`.
  - `factory upstream check <draft>` prints `hash: <12-hex>` when the draft has zero hits (exiting `0`).
  - `factory upstream post <draft> --hash <12-hex>`: add `#[arg(long)] pub hash: Option<String>` on `UpstreamVerb::Post`. Execution order: (1) refuse inside a run (`BOARD_ENV`), (2) screen for leak hits (exiting `1` if any hit), (3) require `--hash` to be present AND equal to `draft_hash(&text)` (otherwise print a clear error naming `factory upstream check <draft>` and exit `1` without calling `gh`).
  - Update `skills/harvest/SKILL.md:38-40` to pass `--hash <hash>` from `factory upstream check` into `factory upstream post <draft> --hash <hash>` and `:61` to drop the outdated "misses homes with no `/`" sentence, and bump crate/bundle/cockpit version `0.18.0` → `0.19.0` (`Cargo.toml`, `Cargo.lock`, `skills/bundle.lock`, `mods/cockpit/`).

**Accepted cost.** Requiring `--hash` on `factory upstream post` adds one flag to the human/skill command (`factory upstream post <draft> --hash <h>`) in exchange for guaranteeing the posted file is byte-for-byte identical to the checked file the owner reviewed.

**Acceptance.** Integration tests (`tests/upstream.rs` updated authorized tests + new `tests/upstream_hardening.rs` using a fresh tempdir `gh` stub) prove: (1) `gh` receives `--repo github.com/emilianotisato/factory` and `--body-file -` with the draft body delivered on stdin (NOT on argv), and `gh_stdin` handles an early-exiting stub without a broken-pipe error; (2) `upstream post` opens a GitHub issue even when `[source]` is set in `config.toml`; (3) remotes named `upstream` / `github` (without `origin`) are screened; (4) `acme-api` catches `AcmeApi`, `acme_api`, `acme api`, and `acme` (reporting one hit per name per line), while `--allow api` lets a repo literally named `api` pass (echoing `allowed: api`) without suppressing email/home-path hits; (5) `upstream check` prints `hash: <12-hex>`, `upstream post` without `--hash` or with a stale hash after editing the draft exits `1` without calling `gh`, and `upstream post --hash <12-hex>` succeeds; (6) `skills/harvest/SKILL.md` includes `--hash` and drops the stale no-trailing-slash caveat, and `tests/skills_bundle` + `claude plugin test mods/cockpit` pass at `0.19.0`.

## 5. FR-3 — `factory upstream pull` triage verb in the factory repo

**Today.** There is no verb in `factory` to pull open upstream GitHub issues from `github.com/emilianotisato/factory` into `docs/ideas/` and close them with a reference comment.

**Decided 2026-11-27.**

- **Add `UpstreamVerb::Pull(PullArgs)` (`src/cli/upstream.rs`, `src/upstream.rs`):**
  - `factory upstream pull` (default / list mode):
    - Queries open issues on `github.com/emilianotisato/factory` via `gh issue list --repo github.com/emilianotisato/factory --state open --limit 50 --json number,title,body,url,labels,createdAt,author`.
    - Prints a concise human-readable list (`#<n>  <title>  <url>`, or `no open upstream issues`) — or with `--json`, prints the JSON array of open issues cleanly for skills/agents.
  - **Triage actions (explicit, non-interactive flags usable by both humans and skills):**
    - `factory upstream pull --idea <NUMBER>` — fetches issue `#<NUMBER>` (`gh issue view <NUMBER> --repo github.com/emilianotisato/factory --json number,title,body,url`), creates a new idea file in `docs/ideas/` via `ideas::add` with `origin = Some(issue.url)`, commits the new idea file by pathspec (`docs: add idea <id> from #<NUMBER>`), then posts a comment via `ci::gh_stdin` (`gh issue comment <NUMBER> --repo github.com/emilianotisato/factory --body-file -`) naming the new idea file path and closes via `gh issue close <NUMBER> --repo github.com/emilianotisato/factory --reason completed`.
    - `factory upstream pull --fold <NUMBER> <IDEA_ID>` — fetches issue `#<NUMBER>`, resolves the existing idea file in `docs/ideas/` matching `<IDEA_ID>`, appends a dated `## From #<NUMBER> (<url>)` section with the issue title and body, commits the updated idea file by pathspec (`docs: fold #<NUMBER> into idea <IDEA_ID>`), comments via `--body-file -` naming the idea file, and closes `#<NUMBER>` (`--reason completed`).
    - `factory upstream pull --drop <NUMBER> --because <REASON>` — requires non-empty `--because`, comments via `--body-file -` with `Dropped: <reason>`, and closes `#<NUMBER>` (`--reason "not planned"`) without modifying `docs/ideas/`.
  - Guardrails:
    - Refuses inside a run (`BOARD_ENV`, same as `check`/`post`).
    - `--idea` and `--fold` require running inside a git checkout that has `docs/ideas/` and refuse if `docs/ideas/` is missing; the local git commit runs BEFORE commenting/closing on GitHub so a failed local commit never closes an issue prematurely.
    - Mutually exclusive flags (`--idea`, `--fold`, `--drop`, `--json` via clap `conflicts_with`).

**Accepted cost.** Commenting via `gh issue comment --body-file -` followed by `gh issue close` is two `gh` calls rather than one `gh issue close --comment` call, keeping comment text off argv consistently with FR-2. Non-interactive action flags avoid brittle TTY loops while working cleanly for both humans and skills.

**Acceptance.** Integration test `upstream_pull_lists_and_triages_issues_to_idea_fold_and_drop` (in `tests/upstream_hardening.rs`, using a fresh tempdir `gh` stub) proves: (1) `factory upstream pull` and `factory upstream pull --json` list open issues from `--repo github.com/emilianotisato/factory`; (2) `factory upstream pull --idea 42` creates `docs/ideas/YYYY-MM-DD-*.md` with `origin: "<issue url>"`, commits it cleanly, comments via `--body-file -`, and closes `#42` (`--reason completed`); (3) `factory upstream pull --fold 43 <id>` appends issue `#43`'s body into the existing idea file, commits it, comments via `--body-file -`, and closes `#43`; (4) `factory upstream pull --drop 44 --because "duplicate"` comments the reason via `--body-file -` and closes `#44` (`--reason "not planned"`) with no git commit; (5) refuses inside a run (`FACTORY_BOARD` set) without calling `gh`.

## 6. FR-4 — Home paths in every shape (including no trailing slash)

**Today.** `src/paths.rs:95-129` (`HOME_PATH`, `ANY_HOME_PATH`, `find_home_path`, `home_paths_to_tilde`) matches ONLY `/(?:home|Users)/[A-Za-z0-9._-]+/` (requiring a trailing `/`). As a result:
- Bare `/home/<name>` followed by space, punctuation, quote, or end-of-line is NOT caught by `W-HOME-PATH` (`src/docs/check.rs:831`), NOT rewritten by `harvest --done` (`src/docs/ship.rs:172`), and NOT caught by `factory upstream check` (`src/upstream.rs:141`).
- WSL `/mnt/<drive>/Users/<name>`, Windows `<drive>:\Users\<name>`, `/root/`, case-insensitive `/users/<name>`, and JSON-escaped `\/home\/<name>` are also missed.

**Decided 2026-11-27.**

- **Extend `paths::find_home_path` and `paths::home_paths_to_tilde` (`src/paths.rs:95-155`):**
  - **Crucial implementation invariant (caught 2026-11-27 in adversarial pass):** NEVER consume a trailing delimiter byte (like space) inside the regex match! Match ONLY the home prefix itself (with optional trailing separator `/` or `\` or `\/`), and validate both the leading boundary (`start == 0 || !in_segment(bytes[start - 1])` for non-binary input) and the trailing boundary (`ends_with_sep || end == bytes.len() || !in_segment(bytes[end])`) by manual byte checks! That ensures adjacent paths separated by a single space (`cd /home/<a_user> /home/<b_user>`) BOTH match and rewrite cleanly!
  - Supported shapes:
    1. Unix/macOS (case-insensitive `home`/`users`): `/(?:home|users)/[A-Za-z0-9._-]+/?` (bare `/home/` and `/Users` with no username stay non-hits).
    2. WSL (case-insensitive `users`): `/mnt/[A-Za-z]/users/[A-Za-z0-9._-]+/?`.
    3. Windows (case-insensitive `users`): `[A-Za-z]:\\users\\[A-Za-z0-9._-]+\\?`.
    4. Root home (trailing slash required so bare `chroot /root` does not over-trigger): `/root/` (leading boundary already excludes `https://example.com/root/`).
    5. JSON-escaped Unix/macOS (case-insensitive `home`/`users`): `\\/(?:home|users)\\/[A-Za-z0-9._-]+(?:\\/)?`.
  - In `home_paths_to_tilde(text: &str) -> String`:
    - Trailing `/` → `~/` (e.g. Unix/macOS/WSL `/home/<name>/file` → `~/file`, `/root/file` → `~/file`).
    - No trailing separator → `~` (preserving the trailing space/punctuation/EOL untouched!).
    - Trailing `\/` (JSON-escaped) → `~\/` (and no trailing `\/` → `~`).
    - Trailing `\` (Windows) → `~\` (and no trailing `\` → `~`).
- **Update `src/paths.rs:209` unit test** to assert bare `/home/<name>` followed by space IS a hit (`Some(0)`), while `/home/` and `/Users` remain `None`, and update `skills/harvest/SKILL.md:61` to drop the outdated caveat that `W-HOME-PATH` / `--done` miss homes without a trailing `/`.

**Accepted cost.** Matching bare `/home/<name>` without a trailing slash means tracked docs in `docs/` must use placeholders like `/home/<name>` (not concrete usernames), which is intentional so real home paths never slip into archived docs or upstream issues.

**Acceptance.** Unit & integration test `home_paths_catch_and_rewrite_no_trailing_slash_wsl_windows_root_and_json` proves: (1) `find_home_path` catches bare Unix home before space/EOL/quote, adjacent paths separated by a single space, lowercase `/users/<name>/...`, WSL `/mnt/c/Users/<name>/...`, Windows `C:\Users\<name>\...`, `/root/.bashrc`, and JSON-escaped `\/home\/<name>\/...`, while returning `None` for `/home/`, `/Users`, `/tmp/home/<name>/`, `~/home/<name>/`, and `https://example.com/root/a`; (2) `home_paths_to_tilde` rewrites all shapes accurately (including two adjacent bare home paths on one line separated by one space!); (3) `factory docs check` warns `W-HOME-PATH` on a fixture doc containing a bare Unix home path, `factory upstream check` rejects it, and `factory docs check` on this repo itself passes with 0 errors and 0 `W-HOME-PATH` warnings.

## 7. Decisions log

No `OPEN-n`. Every fork was settled on 2026-11-27:

- Keep tag-triggered stable releases (`v*.*.*`) and add a rolling `edge` prerelease workflow (`.github/workflows/edge.yml` with `fetch-depth: 0`, per-job `Cargo.toml`/`Cargo.lock` patch, direct `gh release create --prerelease`, and pruning older `*-edge.*` prereleases); `factory upgrade --edge` uses `UpdateRequest::LatestMaybePrerelease` (§3).
- Check receipt version vs `CARGO_PKG_VERSION` in `from_release()`, let `factory upgrade` install from GitHub Releases even with no receipt when `[source]` is unset (using `install_prefix_from_exe` stripping `/bin` for cargo-home layout), and retire the `cargo install` rule from `CLAUDE.md`/`AGENTS.md` (§3).
- Make `factory upstream post` always open a GitHub issue (removing `[source]` shortcut), add `factory upstream pull` (`--json`, `--idea`, `--fold`, `--drop` using stdin `--body-file -` comments), and keep `/harvest` inside the factory repo writing to `docs/ideas/` directly (§4, §5).
- Harden `factory upstream` (`github.com/` host, `--body-file -` stdin ignoring `BrokenPipe`, all remotes, name-side spelling variants emitting one hit per `(line, name)` + `--allow`, required `--hash` from `check`, `skills/harvest/SKILL.md` + `0.19.0` bump) and extend `paths.rs` home-path matching/rewriting with non-consuming boundary checks across all shapes (§4, §6).

## 8. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit` *(declared in `docs/project.toml`)*
**Surface**: `.github/workflows/edge.yml`, `factory upgrade [--check] [--edge]`, `factory upstream check|post|pull`, `factory docs check` (`W-HOME-PATH`), `CLAUDE.md`/`AGENTS.md`/`README.md`, and `skills/harvest/SKILL.md`. No GUI: every FR is proven below it (plus one owner verification of live `edge.yml` on first post-merge push to `main`).
**Harness**: `tests/upgrade.rs`, `tests/release.rs`, `tests/upstream.rs`, `tests/upstream_hardening.rs` (new), `tests/skills_bundle.rs`, `src/cli/upgrade.rs`, `src/paths.rs`, `src/upstream.rs`, `mods/cockpit/tests/cockpit.test.tsx`.
**Seed state**: local HTTP stub release server serving stable and prerelease JSON, mismatched-version/missing receipts, tempdir `gh` stub capturing stdin and scripted issue lists/comments/closes, multi-remote git repos, and drafts/docs with all home-path shapes.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `cargo test --test upgrade` and `cargo test --test release` (plus post-merge observation of live `.github/workflows/edge.yml` publishing a prerelease on `main`) | test transcript + post-merge workflow URL | yes (post-merge edge workflow) |
| FR-2 | run `cargo test --test upstream` and `upstream_hardening_host_stdin_remotes_variants_allow_and_hash` | test transcript | no |
| FR-3 | run `upstream_pull_lists_and_triages_issues_to_idea_fold_and_drop` | test transcript | no |
| FR-4 | run `home_paths_catch_and_rewrite_no_trailing_slash_wsl_windows_root_and_json` and `factory docs check` | test transcript | no |

## 9. Slices

_(Not sliced yet. Run `/slice`.)_

## 10. Non-goals

- **Cross-repo ideas scanner (`factory ideas --json` + `/ideas` skill).** Decided out of scope on 2026-11-05; stays in `docs/ideas/2026-10-12-cross-repo-ideas-scanner-factory-ideas-json-plus-a.md`.
- **Moving `dist-workspace.toml` `install-path` out of `CARGO_HOME`.** Not handled (2026-11-27): would break existing users' `PATH`; receipt-version check + receipt-less `factory upgrade` (`install_prefix_from_exe`) solve the issue without changing install path.
- **Forcing `/harvest` inside the `factory` repo itself to round-trip through GitHub issues.** Not handled (2026-11-27): local lessons inside the factory checkout still write directly via `factory docs idea add`; all other repos use `factory upstream post`.

## 11. Verification notes

Checked against `main` at `e446f6d`, 2026-11-27, plus `axoupdater 0.10.2` source (`receipt.rs`, `release/github.rs`, `lib.rs`) and live `v0.18.0` `factory-installer.sh:595-600`; revised 2026-11-27 after cross-vendor adversarial pass (`claude:opus`, log `20261127-165828`). **Re-verify at slice entry.**

- `dist-workspace.toml:1-23` (`cargo-dist-version = "0.32.0"`, 4 Unix targets, `install-path = "CARGO_HOME"`, `install-updater = false`).
- `.github/workflows/release.yml:1-296` (tag trigger, `dist build`, `dist host`, `gh release create`).
- `src/cli/upgrade.rs:1-100` (`UpgradeArgs`, `updater`, `from_release`, `source_line`, `ahead_hint`, `upgrade`).
- `src/ci.rs:66-85` (`exec`, `gh`, `git` — needs `gh_stdin` with `BrokenPipe` tolerance).
- `src/upstream.rs:1-202` (`REPO`, `parse`, `names`, `homes`, `holds_word`, `hits`, `search`), `:245-257` (unit tests).
- `src/cli/upstream.rs:1-161` (`UpstreamVerb`, `upstream`, `screen`, `check`, `post`, `add_idea`, `open_issue`).
- `src/paths.rs:90-155` (`in_segment`, `HOME_PATH`, `ANY_HOME_PATH`, `find_home_path`, `home_paths_to_tilde`), `:203-253` (unit tests, `:209` pins bare home path as `None`).
- `CLAUDE.md:6`, `AGENTS.md:6`, `README.md:99-107`, `skills/harvest/SKILL.md:34-41, :61`.
- `tests/upgrade.rs:1-187`, `tests/upstream.rs:1-203`, `tests/release.rs:1-72`, `tests/skills_bundle.rs`, `mods/cockpit/tests/cockpit.test.tsx:5`.
- `axoupdater 0.10.2` API verified: `AxoUpdater::set_release_source`, `set_install_dir` (sets `FACTORY_INSTALL_DIR` → `_install_layout="cargo-home"` which appends `/bin`, so `install_prefix_from_exe` must strip trailing `bin`), `configure_version_specifier(UpdateRequest::Latest | UpdateRequest::LatestMaybePrerelease)`, `query_new_version`, `run_sync`; prerelease list endpoint `/repos/{owner}/{name}/releases`; receipt path order in `receipt.rs:123-171`.
