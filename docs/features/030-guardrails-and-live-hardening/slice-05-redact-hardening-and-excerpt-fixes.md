# Slice 05 — Live redactor hardening and excerpt/gate_failure fixes

**Source**: [`prd.md`](prd.md) → §7 (FR-5). **Depends on**: — _(independent of Slices 01–04)_
**Created**: 2026-11-27

**Progress**: checkpoint prove: `cp-1/FR-5/transcript.txt`.

**Slice**: Hardens `live/redact.rs` with high-signal secret rules (URL userinfo, auth/cookie headers, PEM private keys, additional vendor token prefixes), fixes `bypass=`/`compass=` over-redaction, exempts functional `evidence[].path` and `attention[].answer` fields from token rewriting, and fixes `excerpt.rs` JSON `"failed": 0` false hits, colon-SGR `38:2:<cs>:r:g:b` false-red detection, and empty-matching `gate_failure` regexes in `docs/check.rs` and `docs/project.rs`.

## Current code / environment

- **`live/redact.rs`**: `src/live/redact.rs:15-68` — `RULES` (`:15-42`), `all` (`:45-55`), `walk` (`:57-68`).
  - Line 34: `r#"(?i)\b(\w*(?:key|secret|token|password|pass))=(?:"[^"]*"|'[^']*'|[^\s"'&,;]+)"#` matches `bypass=0` and `compass=north`!
  - `walk` passes only `tilde: bool` and applies `tokens()` to every string, including `evidence.checkpoints[*].files[*].path` and `attention[*].answer` (where a 41-char path segment or flag gets corrupted by `/[0-9A-Za-z]{41,}`!).
- **`excerpt.rs`**: `src/excerpt.rs:41-42, :145-147, :194-227`.
  - `ZERO_FAILED` (`:42`) matches only `\b0 failed\b`, NOT JSON `"failed"\s*:\s*0\b`.
  - `has_red` (`:208-222`) splits on `[';', ':']` together and advances `k += 5` for `38:2`, so 6-field `38:2::0:0:31` reads `31` as SGR red!
- **`docs/check.rs` & `docs/project.rs`**:
  - `src/docs/check.rs:257-270`: compiles each `gate_failure` regex string, does not reject `re.is_match("")`.
  - `src/docs/project.rs:41` (`Project::failure_patterns`): compiles and filters `gate_failure` patterns, does not skip `re.is_match("")`.
- **Deliberate non-changes** — *(prd §11 Non-goals)*:
  - Do NOT add double-encoded JSON, multi-line JSON, percent-encoded token tails, or `HOME`-unset redactor workarounds.
  - Do NOT change excerpt first-hit vs last-20 title, `}{` JSON split, `\r` split, raw CI log storage, or exclusive tiers.

## Tasks

### 1. Harden `src/live/redact.rs` secret rules, fix `bypass=` over-redaction, and exempt `evidence[].path` & `attention[].answer` — high — blocked by: —
**Source**: §7 FR-5: "Harden `src/live/redact.rs`".

`src/live/redact.rs`.
1. Add high-signal rules to `RULES` (`:15-42`):
   - URL userinfo: `(r"(?i)([a-z][a-z0-9+.-]*://[^/\s:@]+:)[^@\s/]+(@)", format!("${{1}}{m}${{2}}"))`
   - `Authorization: Basic/Token`: `(r"(?i)\b(authorization\s*:\s*(?:basic|token)\s+)\S+", format!("${{1}}{m}"))`
   - `PRIVATE-TOKEN:` header: `(r"(?i)\b(private-token\s*:\s*)\S+", format!("${{1}}{m}"))`
   - `Cookie:` / `Set-Cookie:` header: `(r"(?i)\b((?:set-)?cookie\s*:\s*)[^\r\n;]+", format!("${{1}}{m}"))`
   - PEM private key blocks: `(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z0-9 ]*PRIVATE KEY-----|$)", m.to_string())`
   - Vendor prefixes: `\bsk-proj-[A-Za-z0-9_-]+`, `\bglpat-[A-Za-z0-9_-]+`, `\bnpm_[A-Za-z0-9]+`, `\bAIza[0-9A-Za-z_-]{20,}`, `\bsk_(?:live|test)_[0-9A-Za-z]+`, `\bxapp-[0-9A-Za-z-]+`.
2. Fix the env assignment rule (`:34`) to require word-start or underscore boundary before the sensitive keyword:
   `r#"(?i)\b((?:[A-Za-z0-9]+_)*(?:key|secret|token|password|pass))=(?:"[^"]*"|'[^']*'|[^\s"'&,;]+)"#`
   Verify it still matches `DB_PASSWORD=x`, `API_KEY=x`, `TOKEN=x`, `pass=x`, `my_secret=x`, and does NOT match `bypass=0` or `compass=north`!
3. Update `all` / `walk` (`:45-68`) to skip `tokens()` (while keeping `tilde` rewrite if enabled — or leaving functional `answer` intact) on:
   - `path` inside `evidence` (i.e. `evidence.checkpoints[*].files[*].path`)
   - `answer` inside `attention` (i.e. `attention[*].answer`)

**The trap**: Keep all existing unit tests in `src/live/redact.rs` passing! In particular, make sure `evidence[].path` still gets `~` home-prefix rewriting if existing tests expect tilde on paths, while skipping `tokens()` so a 41-char filename or token-like segment never breaks the path! And make sure `attention[].answer` skips `tokens()` so the argv array stays runnable.

**Acceptance:** Unit tests in `src/live/redact.rs` prove every new secret rule redacts, `bypass=0` and `compass=north` stay untouched, and `evidence.checkpoints[0].files[0].path` + `attention[0].answer` are not rewritten by `tokens()`.

### 2. Fix `src/excerpt.rs` JSON `"failed": 0` & colon-SGR `38:2`, and reject empty-matching `gate_failure` regexes — high — blocked by: —
**Source**: §7 FR-5: "Fix `src/excerpt.rs` and `src/docs/check.rs` (`src/docs/project.rs`)".

`src/excerpt.rs`, `src/docs/check.rs`, `src/docs/project.rs`.
1. In `src/excerpt.rs`:
   - Update `ZERO_FAILED` (`:42`) or add `ZERO_JSON_FAILED` (`r#""failed"\s*:\s*0\b"#`) stripped before `FAILED.is_match(...)` in `generic()` (`:145-147`) so `{"result":"passed","failed":0}` is NOT a generic failure (while `{"result":"failed","failed":2}` STILL matches!).
   - Fix `has_red` (`:207-223`): split `line[start..end]` on `';'` FIRST (`let items: Vec<&str> = line[start..end].split(';').collect();`). Walk `k`:
     - If `items[k]` starts with `"38:"` or `"48:"` (colon-separated extended color, 5 or 6 sub-fields like `38:2::0:0:31`): `k += 1` (skip the whole colon group in one step!).
     - Else match `items[k]`: `"31" | "91" => return true`, `"38" | "48" => k += if items.get(k + 1) == Some(&"5") { 3 } else { 5 }`, `_ => k += 1`.
2. In `src/docs/check.rs` (`:257-270`): after `Regex::new(item)` succeeds, check `if re.is_match("")` and push an error `format!("docs/project.toml [docs] gate_failure {item:?} matches the empty string")`.
3. In `src/docs/project.rs` (`Project::failure_patterns`, `:41`): filter out patterns where `re.is_match("")` is `true` so even an unchecked `project.toml` can never match every line of a gate log!

**The trap**: Keep `has_red` detecting `\x1b[31m`, `\x1b[1;31m`, `\x1b[91m`, and `\x1b[38;2;0;0;31;31m` (where a real `31` follows the 5-part semicolon `38;2;r;g;b`!), while NOT matching `\x1b[38:2::0:0:31m` or `\x1b[38:2:0:0:0:31m`!

**Acceptance:** Unit tests in `src/excerpt.rs`, `src/docs/check.rs`, and `src/docs/project.rs` prove `"failed": 0` and `38:2::0:0:31` are ignored while real errors/red SGR still match, and `gate_failure = ["^"]` / `[""]` fails `docs check` and is skipped by `Project::failure_patterns`.

### 3. Add integration test in `tests/redact_and_excerpt.rs` — high — blocked by: 1, 2
**Source**: §7 FR-5 Acceptance & §9 QA plan FR-5 row.

`tests/redact_and_excerpt.rs` (new file — keep free of plan-id strings so `factory leaks` stays clean!).
Add `redact_hardening_and_excerpt_gate_failure_edge_cases` proving:
1. `redact::tokens` redacts `postgres://app:s3cr3t@db`, `Authorization: Basic dXNlcjpwYXNz`, `PRIVATE-TOKEN: glpat-123456`, `Cookie: session=abc`, PEM private key block, and `sk-proj-...`/`glpat-...`/`npm_...`/`AIza...`/`sk_live_...`/`xapp-...`, while leaving `bypass=0` and `compass=north` unchanged.
2. `redact::all` leaves `evidence.checkpoints[0].files[0].path` and `attention[0].answer` uncorrupted even when they contain a 41-char alphanumeric path segment (`/aaaa...45chars`).
3. `excerpt::excerpt` returns empty for `{"result":"passed","failed":0}` and for `\x1b[38:2::0:0:31mgreenish\x1b[0m`, and returns a hit for `\x1b[31merr\x1b[0m` and `\x1b[38;2;0;0;31;31merr\x1b[0m`.
4. `factory docs check` exits non-zero and reports the empty-string error when `docs/project.toml` has `gate_failure = ["^"]` or `gate_failure = [""]`.

**Acceptance:** `cargo test --test redact_and_excerpt` passes; reverting any of the rules or guards causes the test to fail.

### 4. Verify — medium — blocked by: 3
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
**Source**: §9 QA plan FR-5 row.
- **FR-5**: run `cargo test --test redact_and_excerpt redact_hardening_and_excerpt_gate_failure_edge_cases -- --nocapture` and keep the transcript. Expected: passes proving all redactor rules/exemptions, `excerpt` JSON/SGR fixes, and `gate_failure` empty-match rejection.

**Acceptance:** after the checkpoint, the FR-5 row has transcript evidence in this slice's **Progress**.
