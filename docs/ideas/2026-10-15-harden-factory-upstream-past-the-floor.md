---
title: "Harden factory upstream past the floor"
state: "open"
added: "2026-10-15"
origin: "PRD-008 polish"
---

The polish security pass on `factory upstream` (src/upstream.rs, src/cli/upstream.rs) found gaps that are real but outside FR-8 as written:

- **The host.** `--repo emilianotisato/factory` resolves against `GH_HOST`. Someone with `GH_HOST` set to a client's GitHub Enterprise sends the search and the issue there. Fix: `--repo github.com/emilianotisato/factory`; the stub-gh tests in tests/upstream.rs assert the bare form and change with it.
- **The body on argv.** `open_issue` passes the body as `--body <text>`: visible in `ps` to other local users, and over about 128 KiB it fails. `--body-file -` with the body on stdin needs `ci::gh` to take stdin.
- **More home shapes.** The matcher in src/paths.rs knows `/home/<n>/` and `/Users/<n>/` at a path start. It misses `/mnt/c/Users/<n>/` (WSL), `C:\Users\<n>\`, `/root/`, `/users/<n>/` (macOS is case-insensitive) and JSON-escaped `\/home\/x\/`. Polish added the person's own `$HOME` (and its canonical form) as a hit, which covers `/var/home/<n>` on Silverblue; other people's homes in those shapes still pass.
- **Every remote, not origin.** `names()` reads only `origin`; a repo whose remote is `upstream` or `github` gets no owner check.
- **Spelling variants of the name.** `holds_word` matches the name as written. For `jdoe/acme-api`, a draft saying "AcmeApi", "acme api" or "acme_api" passes, and so does `acme` alone. Fix: also match the name with `-`, `_` and spaces folded out, and each part of a hyphenated name over a few letters.
- **A common word as the name.** No minimum length or stop-list: a repo called `api`, `web` or `app` hits on nearly every draft, and `post` can never pass. Fix: an override such as `--allow <word>` that `post` echoes, or a stop-list for names under four letters.
- **Check and post are not tied.** Nothing proves the draft posted is the one the person read; a hash printed by `check` and required by `post` would.

Each is cheap. None has bitten; retrigger: the first client-repo `upstream post`, or a second person using it.
