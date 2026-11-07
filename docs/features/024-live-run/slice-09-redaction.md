# Slice 09 — Redaction over every string

**Source**: [`prd.md`](prd.md) → §9 FR-7, §11 slice recon corrections (`repo` keeps its stored path). **Depends on**: 03 (gate and CI logs reach the output through `gate.tail`, `gate.excerpt`, `ci.excerpt`), 05 (block bodies through `attention[].body`), 06 (worker narration). The acceptance plants a secret in each; without those sections the test cannot fail for the right reason. The redaction function itself touches only `src/live/redact.rs`.
**Created**: 2026-11-07

**Progress**: `src/live/redact.rs` walks the finished snapshot and rewrites every string: eleven `LazyLock` rules (prefix tokens keep their prefix, a JWT goes whole, `Bearer`/`X-API-Key:`/env/JSON keep their name, a `/`-led segment of 41+ alnum becomes `/[redacted]`), then `paths::home_to_tilde`, skipped for the top-level `repo` only. Unit tests cover each shape alone, case-insensitive and suffixed env names, a 40-hex sha kept and dotted names left alone. `tests/live/redact.rs` adds `live_redacts_every_string` (13 planted lines in a `run_failed.error`, a red gate log, `ci/1.log`, a block body and 13 worker narrations, with `HOME` set to the fixture base so `repo` sits under it) and `live_keeps_a_git_sha`; both were checked to fail with the redactor switched off. Inner loop: 1 run, 0 red, 1.7 min. Deviation: the token list is read a little wider (AD-13), and lines are cut to 300/160 chars before the redactor sees them, so a token split by that cut can leave part of itself unredacted.
Evidence (FR-7): cp-3/fr-7-redaction.txt

**Slice**: After this slice, every string `factory live` prints passes once through a redactor just before serialization, whatever section it came from: token shapes become a fixed marker and home paths become `~`. Forty-hex git shas survive. This is best effort; screenshots are untouched and stay local on the consumer's side.

## Current code / environment

- **The hook point**: slice 01 calls `redact::all(&mut Value)` on the finished snapshot before `rev`, the cap and printing, with an identity body in `src/live/redact.rs`. Walk the `Value` and rewrite every `String` (object keys are factory's own and need none). Redacting before `rev` keeps `rev` stable across calls; redacting before the cap measures what is printed.
- **No redactor exists**: no token pattern in `src/` today. `regex` is a dependency.
- **Home paths**: `paths::home_to_tilde(text, &paths::home())` (`src/paths.rs:126-…`) rewrites each path under `$HOME` as `~/…` and leaves a mid-path match alone; `paths::home()` (`:79-83`) reads `$HOME`, `/` when unset (and `/` rewrites nothing). `answers::tilde` (`src/answers.rs:251-253`) wraps it. Tests set `HOME` for the binary they run.
- **`repo` keeps its stored path**: prd §3 "`repo` (the path as `state.json` stores it, as `status --json` prints it: `cli/status.rs:218`)", and §11 review corrections "`repo` was called `~`-relative. Status prints the stored path." The home rewrite skips the envelope's top-level `repo`; token shapes still apply to it (§11 slice recon correction). Evidence paths and `evidence.root` are run-relative already (slice 07).
- **Where secrets were found** (prd §9): `run_failed.error`, raw `gh` stderr, CI excerpts and gate logs. They reach `live` through `timeline[].data` and `label` (slice 01), `gate`/`ci` (slice 03), `attention[].title`/`body`/`reason.excerpt` (slice 05), `workers[].narration`/`doing.label` (slice 06).
- **Deliberate non-change**: prd §9 "best effort only, and screenshots can still show tokens. The consumer keeps images local." prd §11 "A transcript viewer" was dropped partly because streams hold secrets; do not widen what is read to redact more.

## Tasks

### 1. The redactor — high — blocked by: —
**Source**: §9 FR-7: "One pass over **every string** in the output, at serialization time, not over a list of fields", the token shapes ("`sk-ant-`, `ghp_`, `github_pat_`, `AKIA…`, `xox?-`, JWT and `Bearer \S+`; case-insensitive env-style `(KEY|SECRET|TOKEN|PASSWORD|PASS|APP_KEY)=…`; JSON `"(password|secret|token|key)": "…"`; `X-API-Key:` headers; URL path tokens of 41 or more hex or base62 characters, which keeps 40-hex git shas"), "Home paths become `~`."

`src/live/redact.rs`. Compile the patterns once (`std::sync::LazyLock`). Replace each secret's value with a fixed marker such as `[redacted]`, keeping the key or prefix that named it (`TOKEN=[redacted]`, `Bearer [redacted]`) so the line still reads. The env-style rule matches the name as a suffix too (`GITHUB_TOKEN=`, `DB_PASSWORD=`). The URL rule applies to a path segment after `/`, of 41+ chars of `[0-9A-Za-z]`. Then the home rewrite, except for the top-level `repo`.

**The trap:** a 40-hex sha appears in `ci.head`, `ci_red.head`, commit events and gate logs; the long-token rule must not touch it (41 or more), and the JWT rule (three base64url segments split by dots, the first starting `eyJ`) must not match dotted version strings or file names.

**Acceptance:** `live_redacts_every_string`: a fixture run plants one of each token shape, plus the fixture `$HOME` path, in a `run_failed.error` event payload, a gate log line the excerpt picks up, a `ci/1.log` line, a block's `log.jsonl` body, and a worker stream's assistant text; the output of `factory live <id> --json` contains none of the planted secret values and no `$HOME` prefix outside `repo`, and `repo` equals the stored path. `live_keeps_a_git_sha`: a `ci_green` with a 40-hex `head` and a URL with a 40-hex path segment come out unchanged, and the same URL with a 41-char segment is redacted. Unit tests in `src/live/redact.rs` cover each shape alone, case-insensitive env names included.

### 2. Verify — medium — blocked by: 1
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 3. Prove — at the checkpoint — blocked by: 2
prd §12 row **FR-7**: run `live_redacts_every_string` and `live_keeps_a_git_sha`, and keep the transcript.

**Acceptance:** after the checkpoint, this slice's Progress names that transcript.
