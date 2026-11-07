---
title: "live redactor: shapes it misses, cuts short or over-redacts"
state: "open"
added: "2026-11-07"
origin: "PRD-024 polish"
---

The `live` redactor (`src/live/redact.rs`, `RULES`) is best effort. The PRD-024 polish review found shapes it misses or damages, beyond FR-7's list:

- **Missed:** URL userinfo (`postgres://app:secret@db`, `https://oauth2:glpat-…@gitlab.com`); YAML/ini (`password: x`, `aws_secret_access_key = x` with spaces around `=`); `Authorization: Basic …`/`token …`; `Cookie`/`Set-Cookie`; `PRIVATE-TOKEN:`; PEM private key blocks; other vendors' prefixes (`sk-proj-`, `glpat-`, `npm_`, `AIza`, `sk_live_`, `xapp-`); Slack/Discord webhook paths under 41 chars or containing `-`/`_`.
- **Partial:** an escaped quote inside a quoted env value (`DB_PASSWORD="hun\"ter2"` leaves `ter2"`); dotted JSON keys (`"db.password"`) and non-string JSON values (`{"password":12345}`); double-encoded JSON (`"{\"password\":\"x\"}"`); pretty-printed JSON split across lines (redaction is per line); a percent-encoded tail of `sk-ant-`/`ghp_` (`%5F`); a JWT with `=` padding; a prefix glued to a word character (`X_ghp_…`, no `\b`).
- **Over-redacted:** the env rule `\w*(?:key|secret|token|password|pass)=` hits `bypass=0` and `compass=north`.
- **Functional fields:** an evidence `path` or the `answer` argv that happens to match a shape is rewritten, so the panel cannot open the file or runs a wrong argv. Consider exempting those two fields as `repo` is exempt from the home rewrite.
- `HOME` unset → `paths::home()` is `/`, and nothing is rewritten to `~`.

Each extra is cheap to add with a unit test in `redact.rs`; the risk is false positives in gate logs.
