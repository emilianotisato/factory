---
number: 025
slug: ci-mode-and-economics
title: "Local CI mode and cost-aware gate CI"
type: feature
created: 2026-11-25
author: Emiliano Tisato
summary: |
  Adds a user-level CI mode (`[ci] mode = "local" | "remote"` in `~/.config/factory/config.toml`, overridable on new runs with `--ci`), a one-time preflight ask when unset after upgrade that saves the answer back to `config.toml`, a local closeout path that skips `watch_ci` and stamps `[skip ci]` on pushed closeout commits, and cost-aware CI design and audit guidance in `/gate`. Interviewed, corrected by a cross-vendor adversarial pass, and agreed 2026-11-25; four FRs, no OPENs, ready to slice.
---

# PRD-025 — Local CI mode and cost-aware gate CI

> **Interviewed 2026-11-25.** Every `file:line` below was verified against `main` at `584dec7`, then **re-verified by a cross-vendor adversarial pass (`grok-4.7-high`) on 2026-11-25 which caught seven real breaks**. Each correction is inline and dated.

## 1. Why now

Shipping at `factory` cadence exhausted GitHub Actions monthly minutes on private orgs and froze PRs mid-delivery. Measured across six private `factory` repos over the 30 days ending 2026-11-25:

- **High monthly burn against private allowances.** One org (`prokeycloud`, Team plan, 3,000 included min/mo) burned ~5,334 billed min/mo across three repos (`prokeycloud` ~4,126m over 181 runs, `sii-api` ~758m, `arca-api` ~450m) and hit its $0 spending cap at 3,003 min, blocking CI. Another (`cody-agent`) burned ~1,328 min/mo across 166 runs (~29.5 billed min per 7-job PR run).
- **Low marginal catch rate after local polish.** Across those six private repos in `factory.db`, `watch_ci` recorded **83 `ci_green` runs and 5 `ci_red` runs (~5.7%)** — `prokeycloud` alone spent ~4,100 min/mo to catch one red. Every checkpoint and polish pass already ran the full local gate in a clean lane worktree.
- **No way to skip remote CI in `factory`.** On the normal closeout path (`src/closeout.rs:125-130`), `open_pr` pushes and opens a `--draft` PR (`src/ci.rs:418`), and `watch_ci` (`src/closeout.rs:1315`) polls GitHub Actions checks. *Corrected 2026-11-25 (adversarial pass): an earlier draft called `:125-130` unconditional; `open_pr` returns `Outcome::Done` early at `:1065-1077` on an answered `gh pr create refused` `proceed`, and `watch_ci` returns early at `:1324-1326` with no `pr_url` or `:1330-1341` on `proceed`.* Even a developer on a 20-core workstation cannot tell `factory` to rely on its local gate and stop burning org Actions minutes on draft PR pushes.
- **`/gate` encourages expensive CI YAML.** `skills/gate/SKILL.md:70` recommends *"both, same command, so they cannot drift"*, and §5 audit mode (`:144`) checks only whether CI divergence is documented — not whether workflows run on draft PRs, duplicate CI on `push: [main]`, omit `concurrency` cancel-in-progress, omit `paths-ignore` for `docs/**`, omit dependency caches, hardcode local parallelism on 2-vCPU runners, or enable `xdebug` without collecting coverage.

**What this leaves alone.** Self-hosted runner provisioning (an org infra choice outside `factory`), local workflow emulation (`act` / Dagger), and per-checkpoint gate tiering (`docs/ideas/2026-11-20-gate-allow-a-tier-that-runs-only-at-the-last-check.md`).

## 2. Blast radius

- **Every user upgrading `factory`.** A user whose `~/.config/factory/config.toml` has no `[ci] mode` hits one preflight `ASK-n` on their next **new** `factory run` (or exit 2 on `factory docs preflight`) until they choose `local` or `remote` (or pass `--ci` on the new run). Answering `local` or `remote` writes `config.toml` once and never asks again. *Corrected 2026-11-25 (adversarial pass): an earlier draft said "on the next `factory run`" without distinguishing resume; `find_resumable` (`src/run.rs:314-319`, `src/cli/run.rs:72-90`) resumes an already-open pre-upgrade run before the preflight path at `:101`, so an in-progress run started before upgrade finishes with `ci_mode: None` → `remote` without asking.*
- **Teammates on slower machines.** CI mode lives in user `~/.config/factory/config.toml` and `--ci`, never in `docs/project.toml`, so a fast workstation's `local` setting can never be committed to force a teammate's laptop off remote CI.
- **Locked default branches with required GitHub checks.** In `local` mode, `[skip ci]` stops GitHub Actions from running during build, polish, and owner review; the draft PR stays unmergeable until the owner triggers the required check once (e.g. by marking the PR ready for review on a draft-silent workflow, or pushing/re-running once before merge).
- **Existing tests and `autonomy.md:62`.** `tests/harness/mod.rs:580-629` (`build_env`) runs every fixture with an empty `FACTORY_CONFIG_DIR`, and `tests/run.rs:607-654`, `tests/closeout.rs:2287-2303`, and `tests/preflight.rs:51-152` overwrite `config.toml` without `[ci]` or assert that runs start immediately. Adding `FACTORY_CI_MODE=remote` in `tests/harness/mod.rs` keeps every existing test untouched and green (`assets/autonomy.md:62`); new tests for the unset-config ask simply remove that env var.
- **`skills/bundle.lock`, `Cargo.toml` version, and `tests/skills_bundle.rs`.** Adding `skills/gate/references/ci.md` and editing `skills/gate/SKILL.md` and `skills/deliver/SKILL.md` changes the bundle hash (`src/skills.rs:100-115`). *Corrected 2026-11-25 (adversarial pass): an earlier draft claimed `v0.14.0` was untagged locally; `.git/refs/tags/v0.14.0` exists locally (`24fd465c`), so `tag_state` (`src/skills.rs:175-188`) returns `Tagged::Yes` and `relock` refuses at `0.14.0`.* Therefore `Cargo.toml` **must** bump past `0.14.0` (e.g. `0.15.0`) before `FACTORY_BUNDLE_WRITE=1 cargo test` relocks `skills/bundle.lock`, and `src/skills.rs:FILES` (`:25-93`) must list `gate/references/ci.md`.
- **`skills/deliver/SKILL.md` line budget.** `tests/cockpit.rs:767-798` pins `skills/deliver/SKILL.md` at `< 60` lines (currently 50) and checks **18** unique passages verbatim (`:773-795`). *Corrected 2026-11-25 (adversarial pass): an earlier draft counted 17.*
- **External contracts.** `factory status --json` (`src/cli/status.rs:227`) and `factory live --json` (`src/live/gate.rs:191`) gain a `"local"` value on their existing `ci` state field; `factory wait` (`src/cockpit.rs:86-88`) keeps `finished: <pr_url>` unchanged (`tests/cockpit.rs:226-240`). `knobs_text` (`src/cli/report.rs:732-768`) stays unchanged (`tests/knobs.rs:141`).

## 3. FR-1 — User-level CI mode (`[ci] mode`) and `--ci` flag

**Today.** `src/config.rs:16-21` loads `budget_weighted_tokens`, `workers`, `source`, and `review` from `~/.config/factory/config.toml` (`TOP_KEYS` at `:125`). `src/config.rs:126` (`PROJECT_TOP_KEYS`) loads `run` and `workers` from `docs/project.toml`. `src/config.rs` has no writer — every edit to `config.toml` is manual. `RunArgs` (`src/cli/run.rs:16-29`) accepts `--budget`, `--foreground`, and `--hypothesis`; `RunState` (`src/run.rs:94-126`) persists `budget_weighted_tokens` so detached and resumed drivers see flag overrides.

**Decided 2026-11-25.** Add a user-only CI mode knob with precedence **`--ci` flag → `~/.config/factory/config.toml` (`[ci] mode`) → `FACTORY_CI_MODE` env var → unset**:

- In `src/config.rs`:
  - `CiMode` (`Local` | `Remote`, serde `rename_all = "lowercase"`) and `Ci { pub mode: Option<CiMode> }` (default `None`).
  - Add `"ci"` to `TOP_KEYS` (`:125`) and validate `[ci]` keys against `CI_KEYS = ["mode"]` in `unknown_keys` (`:322-360`). Do **not** add `ci` to `PROJECT_TOP_KEYS` (`:126`).
  - Add `ci: Source` to `Sources` (`:106-109`): `Source::Flag` when `--ci` is passed, `Source::User` when `[ci] mode` is read from `config.toml`, `Source::Default` otherwise.
  - Keep `config::load` / `load_with_project` pure file-only (never reading process env, so unit tests like `src/config.rs:418` `assert_eq!(l.config, Config::default())` stay isolated from the outer env — *clarified 2026-11-25 after adversarial pass*). Add a separate `config::resolve_ci_mode(loaded: &Loaded, flag: Option<CiMode>) -> (Option<CiMode>, Source, Option<String>)` helper that checks `flag`, then `loaded.config.ci.mode`, then env var `FACTORY_CI_MODE` (`local` | `remote`, case-insensitive; any other non-empty value returns a warning string and `None`). `tests/harness/mod.rs` `build_env` (`:609-628`) sets `FACTORY_CI_MODE=remote`.
  - Record `ci` in `config::knobs` JSON (`:218-241`) as `{"value": "local" | "remote" | null, "source": ...}` without altering `knobs_text` (`src/cli/report.rs:732-768`).
  - Add `config::write_ci_mode(config_dir: &Path, mode: CiMode) -> Result<PathBuf>` that creates `config_dir` if absent, updates or appends `[ci]\nmode = "<mode>"\n` while preserving existing comments and unrelated sections, validates the resulting text with `toml::from_str` before writing, and writes via temp file in `config_dir` + rename (`docs/decisions.md#dec-3`).
- In `src/cli/run.rs` and `src/run.rs`:
  - Add `--ci <local|remote>` (`clap::ValueEnum`) to `RunArgs` (`src/cli/run.rs:16-29`).
  - Persist `ci_mode: Option<CiMode>` on `RunState` (`src/run.rs:94-126`, `#[serde(default, skip_serializing_if = "Option::is_none")]`), written once when the run starts (or when the CI-mode ask is answered). Passing `--ci` when resuming an existing run warns on stderr (`--ci ignored: run <id> already exists`, matching `--hypothesis` at `src/cli/run.rs:193-198`) and leaves `ci_mode` unchanged.

**Accepted cost.** A repository cannot enforce `ci = "remote"` in `docs/project.toml`; a repo that needs remote CI on every PR relies on its GitHub required-checks ruleset at merge time or the user passing `--ci remote`. Overriding user config from `project.toml` lost because committing `ci = "local"` in a shared repo breaks teammates on weaker laptops.

**Acceptance.** `config::load` reads `[ci] mode = "local"` and `"remote"`, warns `unknown key \`ci.<k>\` ignored` on unknown `[ci]` keys, and returns `Err(Error::Toml)` on invalid `mode` values; `[ci]` in `docs/project.toml` warns as an unknown key and does not override user config. `write_ci_mode` creates a missing `config.toml`, appends `[ci]` to an existing file without losing comments or `[source]`/`[review]`/`[workers]`, replaces an existing `mode` inside `[ci]` idempotently, and leaves no temp file behind in `config_dir`. `factory run NNN --ci local` persists `ci_mode = "local"` on `state.json` and in `runs.knobs`, survives daemon launch and resume, and a resumed `factory run NNN --ci remote` warns on stderr and keeps `ci_mode = "local"`.

## 4. FR-2 — Preflight asks once when CI mode is unset and persists the answer

**Today.** `preflight::run` (`src/docs/preflight.rs:88-242`) checks repo state only; user-config checks are layered on in `preflight::cli` (`:381-386`) and `cli::run::run` (`src/cli/run.rs:101`) via `preflight::second_opinion`. Notice `preflight::cli` (`:382`) currently returns the `code` computed by `preflight::run` before any user-config lines are appended. When `pf.ask` is non-empty, `cli::run::run` (`:107-129`) creates the run in `Phase::WaitingOwner`, emits `AskOpened` events (`ASK-1`, … in `pf.ask` order), and exits 0. `cli::answer::answer_ask` (`src/cli/answer.rs:248-358`) logs the answer in `log.jsonl`, emits `AskAnswered`, writes `answers/ASK-n.json`, and wakes the daemon — it never edits `config.toml`.

**Decided 2026-11-25.**

- Add `preflight::CI_MODE_ASK_PREFIX` and `pub fn ci_mode(loaded: &config::Loaded, flag: Option<CiMode>, pf: &mut Preflight)` in `src/docs/preflight.rs` (called from `preflight::cli` at `:384` and `cli::run::run` at `:101`, right next to `second_opinion`):
  - Resolve via `config::resolve_ci_mode(loaded, flag)`.
  - When resolved to `Some(m)`: push `pf.ok`: `"ci mode: <m> (<source>)"`.
  - When unresolved (`None`): push `pf.ask` starting with `CI_MODE_ASK_PREFIX` (`"ci mode not set in config.toml"`), explaining both options and their warning plainly:
    - `local` — skips GitHub Actions watch and stamps `[skip ci]` on closeout commits, relying on your local gate (saves org Actions minutes; needs local CPU; locked default branches still need one CI run when you mark the PR ready to merge).
    - `remote` — pushes and watches GitHub Actions on every PR with auto-fix rounds (lighter on your machine; burns org Actions minutes).
    - `proceed` — uses `remote` for this run only without writing `config.toml`.
  - In `preflight::cli` (`:381-419`), recompute the exit code after `ci_mode`: `1` if `!pf.refuse.is_empty()`, `2` if `!pf.ask.is_empty()`, else `0`.
- In `src/cli/answer.rs` (`answer_ask`, `:248-358`):
  - When the matched `AskOpened.text` starts with `preflight::CI_MODE_ASK_PREFIX`:
    - Parse trimmed lowercase `text`:
      - `"local"` or `"remote"` → call `config::write_ci_mode(&paths::config_dir(), mode)?`, persist `run.set_ci_mode(Some(mode))?` (and update `runs.knobs` with `source: user`), print that `config.toml` was updated, then finish logging and waking as usual.
      - `"proceed"` → do **not** touch `config.toml`; persist `run.set_ci_mode(Some(CiMode::Remote))?` for this run only, then finish logging and waking.
      - Anything else → print an error naming the three valid answers (`local`, `remote`, `proceed`), write nothing to `config.toml`, `log.jsonl`, `events.jsonl`, or `answers/`, keep the ask open, and return exit 1.
- In `skills/deliver/SKILL.md`:
  - Mention `--ci local|remote` beside `--budget` on line 13 (keeping the existing `"1. \`factory run NNN\` (add \`--budget"` needle verbatim).
  - Add one line in §2 so `/deliver` presents the CI-mode `ASK-n` with exact choices `local` / `remote` / `proceed` via `AskUserQuestion`. Stay strictly under 60 lines (`tests/cockpit.rs:771`).

**Accepted cost.** A user upgrading `factory` whose first repo has no `.github/workflows/` still answers the one-time question once (about five seconds) and never sees it again on any repo. Passing `--ci` skips the ask on a **new** run; if a run is already parked on an open CI-mode `ASK-n`, `factory answer ASK-n` closes it (`src/scheduler.rs:198-207`) rather than `--ci` on resume (*clarified 2026-11-25 after adversarial pass*). Gating the ask on `.github/workflows/` existing lost because checking workflows adds filesystem coupling to a machine-wide preference. Mutating `config.toml` from `factory answer` over adding a separate `factory config set` verb won because `/deliver` relays `ASK-n` to the owner's phone and resolves it in one tap.

**Acceptance.** With `FACTORY_CI_MODE` unset and no `[ci]` in `config.toml` (on a fixture with no other preflight asks, so the CI ask is `ASK-1`; on a repo with earlier asks it is `ASK-{index+1}`): (1) `factory docs preflight 001` exits `2` (or `1` when `refuse` is non-empty) and prints the CI-mode ask under `!`; (2) `factory run 001` exits `0` and parks in `WaitingOwner` with `ASK-1` naming `local`, `remote`, and `proceed`; (3) `factory answer ASK-1 "bogus"` exits `1`, writes nothing to `config.toml`, `log.jsonl`, `events.jsonl`, or `answers/`, and leaves `ASK-1` open; (4) `factory answer ASK-1 "local"` writes `[ci]\nmode = "local"\n` to `config.toml`, wakes the run with `ci_mode = "local"`, and a subsequent `factory docs preflight 001` exits `0` with `✓ ci mode: local (user)` and no CI ask; (5) `factory answer ASK-1 "proceed"` leaves `config.toml` untouched and wakes the run with `ci_mode = "remote"`; (6) a **new** `factory run 001 --ci local` bypasses the ask without writing `config.toml`.

## 5. FR-3 — Local CI closeout skips remote watch and stamps `[skip ci]`

**Today.** On the normal closeout path (`src/closeout.rs:119-131`), `closeout::run` runs `polish` → `ship` → `open_pr` → `watch_ci`. `ship` commits `docs/features/NNN-slug/` with `commit::message(board, "docs", &format!("ship PRD-{feature}"))` (`:1007`, 50-char subject cap in `src/commit.rs:148-165`; `docs(001): ship PRD-001 [skip ci]` is 33 chars and `docs(001): refresh numbers [skip ci]` is 36 chars, both well inside the cap). `push` (`:1171-1207`) calls `refresh_numbers` (`:1183`), which may commit `retro.md` with `commit::message(board, "docs", "refresh numbers")` (`:1225`), and then runs `git push -u origin <branch>`. `watch_ci` (`:1315-1533`) polls `gh pr checks` every `FACTORY_CI_INTERVAL` seconds (default 60s, grace 5 polls = 5 min) and spawns up to `CI_FIX_ROUNDS` (3) fix workers. `skills/deliver/SKILL.md:45` commits owner-review changes and pushes once without `[skip ci]`.

**Decided 2026-11-25.**

- Resolve effective closeout CI mode as `run.state().ci_mode.or(cfg.ci.mode).unwrap_or(CiMode::Remote)`.
- When effective mode is `CiMode::Local`:
  - `ship()` (`src/closeout.rs:1004-1008`) appends ` [skip ci]` to the commit subject (`docs(NNN): ship PRD-NNN [skip ci]`).
  - `refresh_numbers()` (`src/closeout.rs:1222-1226`) appends ` [skip ci]` to the commit subject (`docs(NNN): refresh numbers [skip ci]`).
  - `open_pr()` (`:1054-1169`) still pushes the feature branch and opens or refreshes the draft PR as today.
  - `closeout::run()` (`:128-130`) skips `watch_ci()` completely: it reads `head = ci::head(&repo)?`, emits `EventKind::CiLocal { head }` (new variant in `src/events.rs`, serialized with `kind: "ci_local"` and `head`), logs `ci: local mode — remote CI skipped at <short>; mark PR ready for review (or push) to run required merge checks`, and returns `Ok(Outcome::Done)`.
- Wire `EventKind::CiLocal { head }` across all event consumers:
  - `src/live/timeline.rs:38` (`tone` → `"info"`).
  - `src/live/label.rs:14-179` (`label` → `"CI local (remote skipped on <short>)"`).
  - `src/live/gate.rs:191-202` (`state` → `"local"`, `head` → `Some(head.clone())`).
  - `src/live/now.rs:190-236` (`is_ci` includes `CiLocal`, AND `ci_wait` explicitly returns `None` on `EventKind::CiLocal { .. }` — *corrected 2026-11-25 (adversarial pass): adding `CiLocal` only to `is_ci` falls through `:193-201` into `:203-217`, which would still match `PrOpened` and falsely report `"CI checks to register on the PR"` during closeout*).
  - `src/cli/status.rs:283-298` (`ci()` maps `CiLocal` → `Some("local")`, visible on `runs[0].ci` in `status --json` once `pr_url` is set).
  - `src/cockpit.rs:86-88` keeps `finished: <pr_url>` unchanged (`CiLocal` hits `_ => return None` at `:115`) so `tests/cockpit.rs:226-240` stays untouched.
- In `skills/deliver/SKILL.md` §3 (steps 3–4, lines 45–46): check `factory status <id> --json`'s `ci` field; when it is `"local"`, append `[skip ci]` to the post-review commit subject before pushing and tell the owner that remote CI was skipped so they know to trigger merge CI (e.g. mark the PR ready for review) before merging. Pin this wording with `assert_unique_passage` in `tests/cockpit.rs` (*added 2026-11-25 after adversarial pass found `/deliver` §3 had no acceptance check*).

**Accepted cost.** If a run is resumed directly at `pr` after an earlier `remote` run already committed `ship` without `[skip ci]` and `refresh_numbers` has no diff, HEAD will not carry `[skip ci]`: the driver never rewrites committed history, and every fresh run goes through `ship`. *Corrected 2026-11-25 (adversarial pass): an earlier draft mis-cited `docs/decisions.md#dec-1`, which is the pathspec-and-branch rule rather than a no-amend rule.* Trusting polish's lane gate over running a second fresh clean-worktree gate saves 5–15 min per feature; the one-time merge CI run on locked branches provides the clean-room check.

**Acceptance.** On a fresh `factory run 001 --ci local --foreground` run: (1) `git log -1 --pretty=%s` on the feature branch ends with `[skip ci]`; (2) `fx.gh_log()` contains `pr create --draft` and zero `pr checks` calls; (3) `events.jsonl` contains `{"kind":"ci_local",...}` and zero `ci_waiting`/`ci_green`/`ci_no_checks` events; (4) `factory status <id> --json` reports `runs[0].ci == "local"`, `factory live <id> --json` reports top-level `ci.state == "local"` (and `now` does not report waiting on CI during closeout), and `factory wait <id>` outputs `finished: <pr_url>`; (5) `deliver/SKILL.md` carries the `--ci`, CI-mode `ASK-n`, and §3 `[skip ci]` passages pinned by `assert_unique_passage` while staying `< 60` lines. A `remote` run keeps today's plain `ship` subject and calls `watch_ci` unchanged.

## 6. FR-4 — Cost-aware CI design and audit in `/gate`

**Today.** `skills/gate/SKILL.md:70` (Round 1) recommends *"both, same command, so they cannot drift"* and treats local-only tiers as an exception. §5 audit mode (`:144`) checks only whether CI divergence is documented; it never audits workflow triggers, runner sizing, caching, or billable-minute leaks. `src/skills.rs:38-46` bundles `gate/SKILL.md`, `gate/references/stacks.md`, and `gate/references/tiers.md`.

**Decided 2026-11-25.**

- Add `skills/gate/references/ci.md` and register `("gate/references/ci.md", include_str!("../skills/gate/references/ci.md"))` in `src/skills.rs:FILES` (`:38-46`):
  - **Design rule:** ask *what cloud CI proves that the local gate did not* (clean-environment reproducibility and the merge status check).
  - **Draft-silent PR CI:** recommend `on: pull_request: types: [opened, reopened, synchronize, ready_for_review]` with `if: github.event.pull_request.draft == false` so `factory`'s draft PRs burn 0 cloud minutes until the owner marks Ready for Review (cutting multi-push PRs to one merge-time check).
  - **Split-tier CI:** heavy tiers (mutation testing, multi-shard browser suites, complexity/CRAP) stay in the local gate (and optional nightly schedule); PR CI runs the fast clean-room core.
  - **CI cost audit checklist** (8 concrete items grounded in real leaks):
    1. Draft PRs running the full suite.
    2. Duplicate `push: [main]` runs immediately after PR-gated merge (and deploy workflows chaining `workflow_run` on a redundant full verify instead of triggering on `push: [main]`).
    3. Missing `concurrency: { group: ..., cancel-in-progress: true }`.
    4. Missing `paths-ignore: ['docs/**', '**.md']`.
    5. Missing dependency/build caches (Composer, npm/pnpm, Cargo, Go).
    6. Hardcoded local parallelism on 2-vCPU cloud runners (`--processes=10` instead of `$(nproc)` / `2`).
    7. Unused coverage extensions (`coverage: xdebug` without `--coverage`).
    8. Heavy local-only tiers or daily cron jobs (mutation, TIA baselines) burning metered private minutes.
- Update `skills/gate/SKILL.md`:
  - §3 Round 1 (`:70`): point to `references/ci.md`; recommend **draft-silent clean-room CI + full local gate** for metered/slow (>2 min) gates, keeping "same command in both" for fast gates.
  - §5 Audit mode (`:144`): expand the CI audit bullet to run the `references/ci.md` checklist against `.github/workflows/*.yml` and report each leak with its cost.
  - §6 Declare and document (`:161`): link `references/ci.md`.
- Bump `Cargo.toml` version past tagged `0.14.0` (to `0.15.0`) and relock `skills/bundle.lock` (`FACTORY_BUNDLE_WRITE=1 cargo test`). Pin the new guidance in `tests/skills_bundle.rs` with `assert_unique_passage`.

**Accepted cost.** Skill-only guidance means CI workflow hygiene is checked during `/gate` and `/gate audit` rather than automatically on every `factory docs preflight`. A Rust YAML linter in preflight lost because workflow topologies (reusable workflows, deploy pipelines, self-hosted matrices) produce noisy false positives that `/gate audit` handles cleanly with judgment.

**Acceptance.** `skills/gate/references/ci.md` exists, is registered in `src/skills.rs:FILES`, `Cargo.toml` is bumped past `0.14.0`, and `cargo test` passes `every_file_under_skills_is_in_the_bundle_byte_for_byte`, `bundle_lock_matches`, and a unique-passage test checking the draft-silent, duplicate-`push: main`, `concurrency`, `paths-ignore`, caching, runner-parallelism, and unused-`xdebug` items in `gate/references/ci.md` and the links in `gate/SKILL.md`. `tests/cockpit.rs` `the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands` remains green.

## 7. Decisions log

No `OPEN-n`. Every fork was settled on 2026-11-25 and lives in its FR:

- Local closeout trusts polish's lane gate and stamps `[skip ci]` on `ship`, `refresh_numbers`, and `/deliver` §3 review commits, over re-running a second clean-worktree gate or omitting `[skip ci]` (§5).
- Unset CI mode opens a preflight `ASK-n` on new runs (regardless of whether `.github/workflows/` exists) whose `local` / `remote` answer persists atomically to `~/.config/factory/config.toml` and `proceed` runs `remote` once, over a log-only ask or a warning-only judge line (§4).
- CI mode is user-only (`config.toml` + `--ci` flag, plus `FACTORY_CI_MODE` env fallback resolved outside `config::load`), never committed in `docs/project.toml`, so teammates on weaker machines are never forced into `local` (§3).
- CI cost guidance lives in `/gate` + `skills/gate/references/ci.md`, over a binary YAML linter in preflight (§6).
- `Cargo.toml` bumps past tagged `0.14.0` to relock `skills/bundle.lock` cleanly without touching `tests/skills_bundle.rs` (§6).

## 8. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit`
**Surface**: `factory` CLI (`docs preflight`, `run`, `answer`, `status`, `live`, `wait`), `~/.config/factory/config.toml`, and bundled skills (`gate`, `deliver`). No UI: every FR is proven below it.
**Harness**: `tests/harness/mod.rs` (`Fixture`), `tests/preflight.rs`, `tests/run.rs`, `tests/closeout.rs`, `tests/cockpit.rs`, `tests/skills_bundle.rs`.
**Seed state**: fixture repo with empty `FACTORY_CONFIG_DIR` and `FACTORY_CI_MODE` removed (for the unset-config ask); fixture `config.toml` with comments and `[source]`/`[review]` (for comment-preserving atomic write); fixture run with `--ci local` and `--ci remote`.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `ci_mode_reads_user_config_flag_and_env_never_project_toml`, `write_ci_mode_preserves_comments_and_updates_idempotently`, `run_flag_ci_persists_across_daemon_and_resume` | test transcript | no |
| FR-2 | run `preflight_asks_when_ci_mode_is_unset_and_exits_two`, `answer_local_writes_config_and_clears_future_preflight`, `answer_proceed_keeps_config_unset_and_runs_remote`, `answer_invalid_ci_mode_refuses_and_keeps_ask_open` | test transcript | no |
| FR-3 | run `local_ci_skips_watch_stamps_skip_ci_and_reports_local_status`, `remote_ci_keeps_plain_ship_subject_and_watches_checks`, `deliver_skill_handles_local_ci_mode` | test transcript | no |
| FR-4 | run `every_file_under_skills_is_in_the_bundle_byte_for_byte`, `bundle_lock_matches`, `gate_skill_carries_ci_economics_guidance`, `the_cockpit_skill_is_under_sixty_lines_and_names_the_five_commands` | test transcript | no |

No row needs a human, so no `qa-guide.md` is generated.

## 9. Slices

_(Not sliced yet. Run `/slice` once agreed.)_

## 10. Non-goals

- **Self-hosted runner setup or management.** Running a Dockerized self-hosted GitHub Actions runner is org infrastructure, not a `factory` CLI verb; `remote` mode already watches self-hosted runners natively.
- **Local GitHub Actions emulation (`act` / Dagger).** Service containers, Playwright, and org secrets make local workflow emulation brittle; `local` mode relies on the repo's declared `gate` instead.
- **`[ci]` override in `docs/project.toml`.** Explicitly excluded 2026-11-25: committing `ci = "local"` in a shared repo breaks teammates on weaker machines. **Reopens when** a repo needs to forbid `local` mode in config rather than through GitHub's required-checks ruleset.
- **Binary `.github/workflows/*.yml` linting in `factory docs preflight`.** Not handled (2026-11-25): `/gate audit` owns workflow cost review with full judgment. **Reopens when** retros show CI workflow regressions recurring after `/gate audit`.
- **Rewriting already-committed `ship` subjects on resume.** Not handled (2026-11-25): if a run previously shipped in `remote` mode and resumes at `pr` in `local` mode with no `refresh_numbers` diff, HEAD is pushed as-is. **Reopens when** a real resumed run burns CI unexpectedly on this path.
- **Not handled: changing CI mode on resume via `--ci`.** Cut 2026-11-25 (§5 cut): passing `--ci` on an existing run warns and keeps the run's recorded mode (matching `--hypothesis`); switching mid-flight has no evidence yet. **Reopens when** a paused run needs its CI mode flipped without stopping and restarting.
- **Retroactively asking on pre-upgrade in-progress runs.** Not handled (2026-11-25): `find_resumable` resumes an open run before preflight runs (`src/cli/run.rs:72-90`), so pre-upgrade runs finish with `remote`. **Reopens when** upgrading mid-run needs a mid-run prompt.
- **Second clean-worktree gate pass before push in `local` mode.** Not handled (2026-11-25): polish already runs the full gate in a clean lane worktree, and `ship` only writes `docs/features/NNN-slug/`. **Reopens when** a `local`-mode PR fails merge CI due to untracked files or a polish-to-ship regression.
- **Per-checkpoint gate tiering (`final = "..."`).** Tracked separately in `docs/ideas/2026-11-20-gate-allow-a-tier-that-runs-only-at-the-last-check.md`.

## 11. Verification notes

Checked against `main` at `584dec7`, 2026-11-25, then re-verified the same day by a cross-vendor adversarial pass (`factory review second --doc`, `grok-4.7-high`, 1277 s, log `~/.local/share/factory/reviews/20261125-174239`). The pass caught seven breaks; every one is corrected inline above. **Re-verify at slice entry.**

- `src/config.rs:16-21` — `Config`; `:106-109` — `Sources`; `:125-131` — `TOP_KEYS`, `PROJECT_TOP_KEYS`, `RUN_KEYS`, `REVIEW_KEYS`; `:135-168` — `load`; `:178-216` — `load_with_project`; `:218-241` — `knobs`; `:322-360` — `unknown_keys`; `:418` — `assert_eq!(l.config, Config::default())`. No config writer exists today.
- `src/cli/run.rs:16-29` — `RunArgs`; `:72-90` — `find_resumable` resume path (returns before `:101`); `:101` — `preflight::second_opinion`; `:107-129` — `pf.ask` loop creating `Phase::WaitingOwner` and `AskOpened` (`ASK-{i+1}`); `:170` — daemon calls `config::load_with_project`; `:193-198` — `hypothesis_ignored` warning pattern on resume.
- `src/run.rs:94-126` — `RunState` fields (`budget_weighted_tokens`, `pr_url`, etc.); `:314-319` — `find_resumable`.
- `src/docs/preflight.rs:88-242` — `preflight::run` (repo-only); `:221-227` — exit code calculation (`1` refuse, `2` ask, `0` ok); `:338, :343, :351` — existing `pf.ask` pushes; `:360-379` — `second_opinion`; `:381-419` — `preflight::cli` returns `code` from `run` without recomputing after user-config hooks.
- `src/cli/answer.rs:248-358` — `answer_ask` (reads `AskOpened.text`, logs `Kind::Ask`, emits `AskAnswered`, writes `answers/ASK-n.json`, calls `wake` at `:344`).
- `src/scheduler.rs:198-207` — parks while `answers::asks_pending` is non-empty.
- `src/closeout.rs:119-131` — `polish` → `ship` → `open_pr` → `watch_ci` (each returns early on park/done); `:1004-1008` — `ship` commit message; `:1065-1077` — `open_pr` `proceed` early return; `:1183-1184` — `push` calls `refresh_numbers` then `ci::push`; `:1222-1226` — `refresh_numbers` commit message; `:1315-1533` — `watch_ci` (early returns `:1324-1326`, `:1330-1341`).
- `src/commit.rs:148-165` — `commit::message` truncates subjects at 50 chars (`docs(001): ship PRD-001 [skip ci]` = 33, `docs(001): refresh numbers [skip ci]` = 36).
- `src/ci.rs:16-20` — `FACTORY_CI_INTERVAL` / `FACTORY_CI_TIMEOUT` env var convention; `:154` — `ci::checks`; `:418` — `gh pr create --draft`.
- `src/events.rs:208-236` — `CiGreen`, `CiRed`, `CiRerun`, `CiFixSpawned`, `CiWaiting`, `CiNoChecks`; `:342-348` — `Event` flattens `EventKind` (`serde(tag = "kind", rename_all = "snake_case")`).
- `src/live/timeline.rs:38` — exhaustive `tone(&EventKind)`; `src/live/label.rs:11-179` — exhaustive `label` (`:137-144` CI arms); `src/live/gate.rs:191-226` — `ci(ctx)`; `src/live/now.rs:190-236` — `ci_wait` & `is_ci`; `src/cli/status.rs:42-46, :70, :227, :283-298` — `RunJson.ci` & `ci()`.
- `src/cockpit.rs:86-88` — `RunFinished` formats `finished: <pr_url>`, pinned verbatim by `tests/cockpit.rs:226-240`; `:115` — non-cockpit `_ => return None`.
- `src/cli/report.rs:732-768` — `knobs_text`, pinned by `tests/knobs.rs:141` (`ends_with("lanes 1 · factory ... · claude 2.1.296")`).
- `src/skills.rs:25-93` — `FILES` list; `:117-227` — `skills/bundle.lock`, `tag_state`, `relock`. **Corrected 2026-11-25:** `.git/refs/tags/v0.14.0` exists locally (`24fd465c`), so `Cargo.toml` must bump past `0.14.0` before relocking `skills/bundle.lock`.
- `tests/harness/mod.rs:580-629` — `build_env` sets `FACTORY_CONFIG_DIR` to an empty dir; `tests/run.rs:609-612`, `tests/closeout.rs:2287-2288`, `tests/preflight.rs:51-152`, `tests/docs_lifecycle.rs:184` run against empty or partial `config.toml` without `[ci]`.
- `tests/cockpit.rs:767-798` — pins `skills/deliver/SKILL.md` `< 60` lines (currently 50) and 18 unique passages (`:773-795`).
- `skills/gate/SKILL.md:70` (Round 1), `:144` (§5 audit), `:161` (§6 declare); `skills/deliver/SKILL.md:13`, `:28`, `:45-46`.
- **Dead ends checked:** `src/cli/config.rs` does not exist; `Cargo.toml` has `toml = "1.1"` and no `toml_edit` dependency, so comment-preserving updates to `config.toml` use targeted line-level patching validated via `toml::from_str` before atomic rename.
