# Slice 02 — Home paths in every shape

**Source**: [`prd.md`](prd.md) → §6 (FR-4). **Depends on**: — (independent; writes only `src/paths.rs` and `tests/home_paths.rs`)
**Created**: 2026-11-27

**Progress**: Replaced `HOME_PATH`/`ANY_HOME_PATH` with one `HOME_PREFIX` regex plus manual leading/trailing `in_segment` checks in `find_home_path` and `home_paths_to_tilde` (Unix/macOS bare, WSL, Windows, `/root/`, JSON-escaped). Added unit tests in `src/paths.rs` and fixture integration test `tests/home_paths.rs`. Inner loop: 2 runs, 1 red (shared `CARGO_TARGET_DIR` served a stale `factory` test dylib — lane build needs `target/lane-2` per `docs/reference/lanes/lane-2.md`), 190 s green. Deliberate red: stubbed `find_home_path` → `None`; integration test failed `Some(0)` vs `None`.

**Slice**: Extends `paths::find_home_path` and `paths::home_paths_to_tilde` to catch and rewrite every home-path shape — bare Unix/macOS paths with no trailing slash, case-insensitive `/users/<name>`, WSL `/mnt/<drive>/Users/<name>`, Windows `<drive>:\Users\<name>`, the Unix root home prefix (`"/" + "root/"`), and JSON-escaped `\/home\/<name>` — without consuming trailing delimiter bytes, so `W-HOME-PATH`, archive rewrites, and `factory upstream check` catch all of them cleanly. Leaves `skills/harvest/SKILL.md:61` (which requires the version bump) to Slice 03.

**Highest-risk**: Boundary matching in `src/paths.rs` — consuming a trailing delimiter byte inside the regex would skip the second of two adjacent bare home paths separated by one space, and over-broad matching would cause false-positive `W-HOME-PATH` warnings on this repo's own tracked `docs/`.

## Current code / environment

- **`src/paths.rs:90-129`**: `in_segment(b: u8)` checks `b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'~')`. `HOME_PATH` and `ANY_HOME_PATH` match only `/(?:home|Users)/[A-Za-z0-9._-]+/` (requiring a trailing `/`). `find_home_path(text: &[u8])` uses `ANY_HOME_PATH` when `text.contains(&0)` (binary files) and `HOME_PATH` otherwise. `home_paths_to_tilde(text: &str)` iterates `HOME_PATH` captures and replaces each match with `~/`.
- **`src/paths.rs:203-212`**: Existing unit test `a_home_path_is_found_only_where_a_path_starts` has line `:209` asserting that a bare Unix home path followed by space (`and /home/ and /Users`) returns `None`. Under FR-4, bare `/home/<name>` followed by space IS a hit (`Some(0)`), while `/home/` and `/Users` remain `None`; line `:209` must be updated.
- **`src/docs/check.rs:816-838`**: `factory docs check` runs `crate::paths::find_home_path(&bytes)` on every tracked file in `docs/` outside `docs/archive/`. Every tracked doc in this repo (verified at slice time) is clean under the new shapes.
- **`src/docs/ship.rs:172`** and **`src/upstream.rs:141`**: Both already call `paths::home_paths_to_tilde` and `paths::find_home_path`, so updating `src/paths.rs` upgrades archive rewrites, `W-HOME-PATH`, and `upstream check` in one place.
- **Deliberate non-change**: Do NOT change `home_to_tilde` (`src/paths.rs:132-155`) or edit `skills/harvest/SKILL.md` in this slice (Slice 03 owns `skills/harvest/SKILL.md` and the `0.18.0` → `0.19.0` bundle bump).

## Tasks

### 1. Extend `find_home_path` and `home_paths_to_tilde` with non-consuming boundary checks — high — blocked by: —
**Source**: §6 FR-4: "NEVER consume a trailing delimiter byte (like space) inside the regex match! Match ONLY the home prefix itself (with optional trailing separator `/` or `\` or `\/`), and validate both the leading boundary (`start == 0 || !in_segment(bytes[start - 1])` for non-binary input) and the trailing boundary (`ends_with_sep || end == bytes.len() || !in_segment(bytes[end])`) by manual byte checks!"

`src/paths.rs`.
- Replace `HOME_PATH` / `ANY_HOME_PATH` with a single `regex::bytes::Regex` (matching ONLY the home prefix itself, never a leading or trailing delimiter!) covering all 5 shapes:
  1. Unix/macOS (case-insensitive `home`/`users`): `(?i:/(?:home|users)/[A-Za-z0-9._-]+/?)`
  2. WSL (case-insensitive `users`): `(?i:/mnt/[A-Za-z]/users/[A-Za-z0-9._-]+/?)`
  3. Windows (case-insensitive `users`): `(?i:[A-Za-z]:\\users\\[A-Za-z0-9._-]+\\?)`
  4. Root home (trailing slash required so bare `chroot /root` does not over-trigger): literal leading slash + `root` + trailing slash (written in Rust source string literal)
  5. JSON-escaped Unix/macOS (case-insensitive `home`/`users`): `(?i:\\/(?:home|users)\\/[A-Za-z0-9._-]+(?:\\/)?)`
- **The trap**: Validate boundaries manually on each match `m` from `find_iter(text)` instead of putting delimiter characters in the regex:
  - Leading boundary (skipped when `text.contains(&0)`): `m.start() == 0 || !in_segment(text[m.start() - 1])`. Note that for JSON-escaped `\/home\/...` inside `"..."`, the preceding byte is `"` (not `in_segment`), and for `https://example.com/root/a` the preceding byte is `m` (`in_segment`), so both behave right. For `/tmp/home/<name>/` and `~/home/<name>/`, the preceding byte is `p` or `~` (`in_segment`), so both are excluded.
  - Trailing boundary: let `matched = &text[m.start()..m.end()]`. If `matched.ends_with(b"/")` or `matched.ends_with(b"\\")`, the trailing separator already terminates the username segment; otherwise require `m.end() == text.len() || !in_segment(text[m.end()])` (so `/home/<name>` at EOL, before space, comma, quote, `)`, etc. matches, while `/home/` and `/Users` never match the regex at all).
- In `home_paths_to_tilde(text: &str) -> String`:
  - Iterate valid boundary-checked matches on `text.as_bytes()`, keeping non-matching slices verbatim:
    - If `matched.ends_with(b"\\/")` → push `"~\\/"`
    - Else if `matched.ends_with(b"/")` → push `"~/"`
    - Else if `matched.ends_with(b"\\")` → push `"~\\"`
    - Else → push `"~"`
- Update `src/paths.rs:209` so the existing bare-home-plus-space input returns `Some(0)`, while `/home/ and /Users` returns `None`.
- Add unit tests in `src/paths.rs` covering every shape, adjacent bare paths separated by a single space, and every negative case (`/home/`, `/Users`, `/tmp/home/...`, `~/home/...`, `chroot /root`, `https://example.com/root/a`, binary NUL prefix).

**Acceptance:** Unit tests in `src/paths.rs` prove all 5 shapes match and rewrite cleanly including adjacent bare paths separated by one space, while all negative cases return `None` (fails if any of the 5 shapes or the non-consuming boundary check is removed).

### 2. Integration test for `W-HOME-PATH` and `upstream check` on new home-path shapes — high — blocked by: 1
**Source**: §6 FR-4 Acceptance: "Unit & integration test `home_paths_catch_and_rewrite_no_trailing_slash_wsl_windows_root_and_json` proves: (1) `find_home_path` catches bare Unix home before space/EOL/quote, adjacent paths separated by a single space, lowercase `/users/<name>/...`, WSL `/mnt/c/Users/<name>/...`, Windows `C:\\Users\\<name>\\...`, Unix root home, and JSON-escaped `\\/home\\/<name>\\/...`, while returning `None` for `/home/`, `/Users`, `/tmp/home/<name>/`, `~/home/<name>/`, and `https://example.com/root/a`; (2) `home_paths_to_tilde` rewrites all shapes accurately (including two adjacent bare home paths on one line separated by one space!); (3) `factory docs check` warns `W-HOME-PATH` on a fixture doc containing a bare Unix home path, `factory upstream check` rejects it, and `factory docs check` on this repo itself passes with 0 errors and 0 `W-HOME-PATH` warnings."

`tests/home_paths.rs` (new).
- Create integration test `home_paths_catch_and_rewrite_no_trailing_slash_wsl_windows_root_and_json` in `tests/home_paths.rs` (using a tempdir fixture git repo — NEVER reading this repo's live `docs/` in a `cargo test`, per `AGENTS.md:5`!):
  1. Directly tests `factory::paths::find_home_path` and `factory::paths::home_paths_to_tilde` across all 5 shapes, adjacent bare paths separated by a single space, and every negative case listed above.
  2. Creates a fixture repo via `factory docs init`, commits a doc containing a bare Unix home path (no trailing `/`), runs `factory docs check`, and asserts stdout/stderr contains `W-HOME-PATH`.
  3. Runs `factory upstream check` on a draft containing a bare Unix home path (no trailing `/`) with a stub `gh` (or fixture `PATH`) and asserts exit `1` with `a home path:`.
- **The trap**: Remember `AGENTS.md:5` ("Tests read fixtures, never this repo's live `docs/`"). Keep `tests/home_paths.rs` 100% fixture-based; running `factory docs check` on this repo itself happens in the Prove task below.

**Acceptance:** `cargo test --test home_paths` passes `home_paths_catch_and_rewrite_no_trailing_slash_wsl_windows_root_and_json` (fails if bare home paths without trailing slash, WSL/Windows/root/JSON shapes, or non-consuming adjacent rewriting are reverted).

### 3. Verify — medium — blocked by: 1, 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
**Source**: §8 QA plan FR-4 row: "run `home_paths_catch_and_rewrite_no_trailing_slash_wsl_windows_root_and_json` and `factory docs check`".

At Checkpoint 1, run `cargo test --test home_paths home_paths_catch_and_rewrite_no_trailing_slash_wsl_windows_root_and_json` AND `factory docs check` on this repo (confirming `docs check: clean` with 0 errors and 0 `W-HOME-PATH` warnings), recording the transcript in **Progress**.

**Acceptance:** Checkpoint transcript in **Progress** confirms `home_paths_catch_and_rewrite_no_trailing_slash_wsl_windows_root_and_json` passes and `factory docs check` prints `docs check: clean` (would fail if any home-path shape failed to match/rewrite or caused a false positive on tracked `docs/`).
