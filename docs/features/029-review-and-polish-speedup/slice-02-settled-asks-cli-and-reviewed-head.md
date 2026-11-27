# Slice 02 — Answered asks, factory review settled, and state reviewed_head

**Source**: [`prd.md`](prd.md) → §5 (FR-3b `state.toml` & `settled::last_reviewed`) and §6 (FR-4a/4b). **Depends on**: — _(independent of Slice 01 — touches completely disjoint files; lands before Slice 04)_
**Created**: 2026-11-26

**Progress**: _(written as it lands — what was built, the gate numbers, what a review caught)_

**Slice**: Adds optional `reviewed_head` to `state.toml` (cleared automatically when `reviewed` is cleared) and prefers it in `settled::last_reviewed` (with strict `^reviewed =` legacy fallback); includes answered `Kind::Ask` entries in `settled::decisions`; and exposes `factory review settled <NNN>` to print a feature's settled block directly on stdout.

## Current code / environment

- **`state.toml` schema**: `src/docs/state.rs:26-61, :96-114, :165-226` — `State` and `Raw` (`#[serde(deny_unknown_fields)]`) hold `status`, `reviewed`, `shipped`, `slices`. `apply()` handles `"reviewed"`, `"shipped"`, `"status"`, `"slices.*"`. `unknown(key)` lists known keys.
- **`settled::last_reviewed` & `settled::decisions`**: `src/settled.rs:15-93`.
  - `last_reviewed` (`:32-46`) currently checks `state::read(&dir).ok()??.reviewed?` and runs `git log -1 --format=%H -G ^reviewed -- <file>`.
  - `decisions` (`:49-93`) currently filters `.filter(|(_, e)| e.kind == log::Kind::Decision)` — skipping `log::Kind::Ask`.
- **`cli::review::ReviewVerb`**: `src/cli/review.rs:17-23, :70-75` — currently has `Second(SecondArgs)` and `Collect(CollectArgs)`.
- **Existing tests (NOT authorized to modify)**:
  - `tests/review.rs:1150` (`last_reviewed_commit_is_the_one_that_set_reviewed`) — sets `reviewed=2026-11-12` WITHOUT `reviewed_head` and expects `last_reviewed` to return the commit that set `reviewed`.
  - `tests/review.rs:1165` (`clearing_reviewed_asks_for_the_whole_branch`) — sets `reviewed=2026-11-12` then `reviewed=` and expects `last_reviewed` to return `None`.
  - `src/docs/state.rs:319-320` — asserts `e.contains("unknown key `owner`")` (does not pin the exact known-keys list).
- **Deliberate non-changes** — *(prd §10 & Slice boundary)*:
  - Do NOT touch `src/collect.rs`, `src/closeout.rs`, `src/review.rs`, or `skills/polish/SKILL.md` in this slice (those live in Slices 01, 03, and 04 so parallel lanes never conflict).

## Tasks

### 1. Add `reviewed_head` to `state.toml` and prefer it in `settled::last_reviewed` — high — blocked by: —
**Source**: §5 FR-3b: "Add `pub reviewed_head: Option<String>` to `state::State` and `Raw`", "Clearing `reviewed=\"\"` also clears `s.reviewed_head = None`", "In `settled::last_reviewed`: when `state.reviewed.is_some()`, return `state.reviewed_head` if present; otherwise keep the `git log -1 --format=%H -G '^reviewed ='` fallback".

`src/docs/state.rs`, `src/settled.rs`.
1. In `src/docs/state.rs`:
   - Add `pub reviewed_head: Option<String>` to `State` (`:26-32`) and `reviewed_head: Option<String>` to `Raw` (`:55-61`), wired in `parse()` (`:88-94`) and `render()` (`:99-105`, printed right after `reviewed` when `Some`).
   - Add `is_sha(v: &str) -> bool` requiring `(7..=40).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_hexdigit())`.
   - In `apply()` (`:175-179`):
     - `"reviewed" => { s.reviewed = date(value)?; if s.reviewed.is_none() { s.reviewed_head = None; } }`
     - `"reviewed_head" => s.reviewed_head = if value.is_empty() { None } else if is_sha(value) { Some(value.to_string()) } else { return refuse(format!("{key}={value}: not a commit sha (7-40 hex chars)")); }`
   - Update `unknown(key)` (`:224`) to include `reviewed_head`.
   - Add unit tests in `src/docs/state.rs` for setting/clearing `reviewed_head`, rejecting non-hex/short values, and verifying `reviewed=""` clears `reviewed_head`.
2. In `src/settled.rs` (`last_reviewed`, `:32-46`):
   - Read `let st = state::read(&dir).ok()??; st.reviewed.as_ref()?;`.
   - If `let Some(head) = st.reviewed_head.filter(|h| !h.is_empty())`, return `Some(head)`.
   - Otherwise run `git log -1 --format=%H -G '^reviewed =' -- <file>` (strict `^reviewed =` so a commit changing only `reviewed_head` is never misread as setting `reviewed`).

**The trap**: `reviewed=""` MUST clear `s.reviewed_head = None`, otherwise clearing `reviewed` would leave a stale `reviewed_head` behind in `state.toml`! Also keep `st.reviewed.as_ref()?` checked BEFORE returning `st.reviewed_head`.

**Acceptance:** Unit tests in `src/docs/state.rs` and `src/settled.rs` prove `reviewed_head` round-trips, rejects bad SHAs, clears on `reviewed=""`, takes precedence over `git log` in `last_reviewed`, and falls back cleanly when absent (fails if `reviewed_head` is ignored or not cleared on `reviewed=""`).

### 2. Include answered `Kind::Ask` entries in `settled::decisions` and add `factory review settled <NNN>` — high — blocked by: —
**Source**: §6 FR-4: "Include answered asks in `settled::decisions` (`src/settled.rs:49-93`)", "Add `factory review settled <NNN>` (`src/cli/review.rs`)".

`src/settled.rs`, `src/cli/review.rs`.
1. In `src/settled.rs` (`decisions`, `:49-93`):
   - Allow both `log::Kind::Decision` and `log::Kind::Ask` in the main iterator (still respecting `overturned` / `**Supersedes:**`).
   - For `log::Kind::Ask`: look up the latest `log::Kind::Answer` entry with `r.reference.as_deref() == Some(id.as_str())`. If none exists (unanswered ask), return `None` (skip it). If found, format:
     - when `r.body.trim().is_empty()`: `format!("- {id} — {}: answered", e.title.trim())`
     - otherwise: `format!("- {id} — {}: answered — {}", e.title.trim(), r.body.trim().replace('\n', " "))`
   - Add unit tests in `src/settled.rs` beside `a_decision_the_owner_changed_shows_the_change` proving answered asks appear and unanswered asks are omitted.
2. In `src/cli/review.rs`:
   - Add `/// Print a feature's settled decisions and non-goals block` / `Settled { feature: String }` to `ReviewVerb` (`:17-23`) and dispatch it in `review()` (`:70-75`).
   - Implement `fn settled_block(feature: &str, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32>`:
     - Resolve `cwd = std::env::current_dir()?`, `root = paths::repo_root(&cwd)?`.
     - Resolve `dir = match crate::retro::feature_dir(&root, feature) { Ok(d) => d, Err(e) => { writeln!(err, "{e}")?; return Ok(1); } }`.
     - If `let Some(block) = crate::settled::block(&dir)`, write `block` to `out`.
     - Return `Ok(0)`.

**The trap**: A feature that exists with no decisions, no Decisions log, and no Non-goals returns `settled::block(&dir) == None` — that is valid and MUST exit `0` with empty stdout (only an unknown feature exits `1`). Also collapse newlines in multi-line answer bodies to single spaces so one ask stays one `- AD-n — ...` bullet line!

**Acceptance:** `factory review settled 001` prints the exact settled block including answered asks and exits `0`; an unknown feature `999` exits `1`; unanswered asks are absent from stdout (fails if `Kind::Ask` is skipped or if `Settled` verb exits non-zero on empty block).

### 3. Add integration tests in `tests/settled_and_state.rs` — high — blocked by: 1, 2
**Source**: §5 FR-3b Acceptance (2) & §6 FR-4 Acceptance (1)–(2).

`tests/settled_and_state.rs` (new file).
Add `settled_includes_answered_asks_and_prints_via_cli` (and `reviewed_head_controls_last_reviewed_and_clears_with_reviewed`) proving:
1. A feature log with a `Kind::Decision`, an answered `Kind::Ask` (`Kind::Answer` referencing it), and an unanswered `Kind::Ask` renders the decision and `- AD-n — <title>: answered — <answer>` in `factory review settled 001` stdout (matching `factory::settled::for_feature` byte-for-byte) while omitting the unanswered ask; `factory review settled 999` exits `1`.
2. Setting `reviewed=2026-11-26` + `reviewed_head=<sha>` via `factory docs set` makes `factory::settled::last_reviewed` return `Some(<sha>)` even after additional commits land; clearing `reviewed=` clears `reviewed_head` and returns `None`; invalid `reviewed_head=not-a-sha` exits non-zero.

**Acceptance:** `cargo test --test settled_and_state` passes; removing `Kind::Ask` handling or `reviewed_head` lookup causes the test to fail.

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §8 QA plan FR-4 (settled half) and FR-3 (`reviewed_head` half).
- **FR-4 (settled)**: run `cargo test --test settled_and_state settled_includes_answered_asks_and_prints_via_cli -- --nocapture` and keep the transcript. Expected: passes proving answered asks in settled block and `factory review settled <NNN>` CLI output/exit codes.

**Acceptance:** after the checkpoint, the FR-4 (settled) row has transcript evidence in this slice's **Progress**.
