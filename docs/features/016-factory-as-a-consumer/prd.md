---
number: 016
slug: factory-as-a-consumer
title: "Factory as a consumer"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-11-05
author: Emiliano Tisato
summary: |
  The owner runs factory the way any user does: no cargo install in this repo, a release build after each push, and a `factory upgrade` that finds and installs the right build. Ideas from other repos reach this one only as GitHub issues, and a pull verb here triages them into docs/ideas and closes them. Draft from the 2026-11-05 idea triage, not interviewed.
---

# PRD-016 — Factory as a consumer

> **Draft from the 2026-11-05 idea triage.** The owner chose what is in and out, group by group; nothing here is interviewed or verified yet. `/prd` continues this draft: interview, recon, FRs, QA plan. `file:line` facts are from `main` at `ca128f7`; re-verify them.

## 1. Why now

Owner, 2026-11-05: "i want to start being a consumer here, i want to feel what users feel". Today the owner runs `cargo install --path . --locked` after each merge (`CLAUDE.md`), and `/harvest` files factory lessons straight into this repo when it is the factory checkout, so the release, upgrade and issue paths users hit are never walked by the person who owns them. `factory upgrade` cannot tell a cargo install from a release install (`src/cli/upgrade.rs:22`, `dist-workspace.toml:15`); `README.md:107` documents a workaround.

## 2. Candidate FRs

Kept by the owner at the triage. `/prd` turns each into an FR or cuts it.

- **C-1 — Release-only installs here.** The `CLAUDE.md` "after a merge, run cargo install" rule goes. The installer-receipt gotcha on this machine is the bug to fix, not a rule to keep.
- **C-2 — A smart `factory upgrade`.** It finds a newer release, picks the build for this OS and arch, and handles a binary that came from `cargo install` or has no installer receipt.
- **C-3 — Upstream is always a GitHub issue.** `/harvest` §2 and `factory upstream` lose the local shortcut in the factory checkout.
- **C-4 — A pull verb in the factory repo.** It lists open upstream issues and walks a triage with the owner: new idea (origin = the issue URL), fold into an existing idea, or drop. It closes each issue with a comment saying where it went.
- **C-5 — Harden `factory upstream`, now that it is the only road:** host hardcoded (`src/upstream.rs:10`), body on argv (`src/cli/upstream.rs:141-142`), `origin` only (`src/upstream.rs:53`), spelling variants, common-word names, check and post tied by hash.
- **C-6 — Home paths in every shape**, including `/home/<name>` with no trailing slash (`src/paths.rs:204` pins it as no hit).

**Open at the interview:** a release per push or per tag, and its CI cost; who runs `scripts/release.sh` once the owner never cargo-installs.

## 3. Not in scope (decided 2026-11-05)

- **Cross-repo ideas scanner** — stays an idea: client repos' own ideas, not factory's.

## 4. Source ideas

The full text of each idea this draft absorbed; the idea files are deleted.

### factory upgrade cannot tell a cargo install from a release install in ~/.cargo/bin

_added 2026-10-13, origin: PRD-004 polish_

**Found by:** PRD-004 polish, local review and the second opinion independently (2026-10-13).

`dist-workspace.toml` sets `install-path = "CARGO_HOME"`, so the release installer writes `~/.cargo/bin/factory` and a receipt whose `install_prefix` is `~/.cargo`. `cargo install --path .` writes the same file. `axoupdater` 0.10.2's `check_receipt_is_for_this_executable` (`receipt.rs:97-113`) drops the trailing `bin` and compares directories, so once the installer has run on a machine, a later source build still "matches" the receipt.

What that breaks (`src/cli/upgrade.rs` `updater`/`from_release`):
- FR-8's clarification promises "a `cargo install` binary beside a release install prints the source line". With one shared path there is no "beside": `factory upgrade` treats the source build as a release install, and doctor's `ahead` hint says `factory upgrade` instead of `cargo install --path <source>`.
- No downgrade happens: `upgrade` sets the running binary's version (`set_current_version`), so it replaces the build only with a strictly newer release. The cost is a misleading hint, and a dev build silently replaced by a newer release.

Options:
1. Treat the receipt as ours only when its `version` equals `CARGO_PKG_VERSION` (read `factory-receipt.json` ourselves; `axoupdater` has no getter). A same-version source build is then indistinguishable, but harmless.
2. Release into its own dir (`install-path = "~/.factory/bin"`), which changes PATH setup for users.
3. Accept and document in README `## Trying it`.

### Harden factory upstream past the floor

_added 2026-10-15, origin: PRD-008 polish_

The polish security pass on `factory upstream` (src/upstream.rs, src/cli/upstream.rs) found gaps that are real but outside FR-8 as written:

- **The host.** `--repo emilianotisato/factory` resolves against `GH_HOST`. Someone with `GH_HOST` set to a client's GitHub Enterprise sends the search and the issue there. Fix: `--repo github.com/emilianotisato/factory`; the stub-gh tests in tests/upstream.rs assert the bare form and change with it.
- **The body on argv.** `open_issue` passes the body as `--body <text>`: visible in `ps` to other local users, and over about 128 KiB it fails. `--body-file -` with the body on stdin needs `ci::gh` to take stdin.
- **More home shapes.** The matcher in src/paths.rs knows `/home/<n>/` and `/Users/<n>/` at a path start. It misses `/mnt/c/Users/<n>/` (WSL), `C:\Users\<n>\`, `/root/`, `/users/<n>/` (macOS is case-insensitive) and JSON-escaped `\/home\/x\/`. Polish added the person's own `$HOME` (and its canonical form) as a hit, which covers `/var/home/<n>` on Silverblue; other people's homes in those shapes still pass.
- **Every remote, not origin.** `names()` reads only `origin`; a repo whose remote is `upstream` or `github` gets no owner check.
- **Spelling variants of the name.** `holds_word` matches the name as written. For `jdoe/acme-api`, a draft saying "AcmeApi", "acme api" or "acme_api" passes, and so does `acme` alone. Fix: also match the name with `-`, `_` and spaces folded out, and each part of a hyphenated name over a few letters.
- **A common word as the name.** No minimum length or stop-list: a repo called `api`, `web` or `app` hits on nearly every draft, and `post` can never pass. Fix: an override such as `--allow <word>` that `post` echoes, or a stop-list for names under four letters.
- **Check and post are not tied.** Nothing proves the draft posted is the one the person read; a hash printed by `check` and required by `post` would.

Each is cheap. None has bitten; retrigger: the first client-repo `upstream post`, or a second person using it.

### W-HOME-PATH and the archive rewrite miss a home with no trailing slash

_added 2026-10-15, origin: PRD-008 polish_

`find_home_path` and `home_paths_to_tilde` (src/paths.rs) match `/home/<name>/` and `/Users/<name>/` only, as FR-3 and FR-5 of PRD-008 specified, and a unit test pins that `/home/x and` is no hit. A home at the end of a word slips through both: `docs/archive/002-docs-port/prd.md:287` quotes a `find` over the owner's home dir with a space after the name, so `docs check` never warns, `harvest --done` keeps it, and once archived the line is frozen.

The cross-vendor review in PRD-008 polish found it. The qa-guide for the first harvest now has the owner grep for these by hand before `--done`.

Fix: also match `/(home|Users)/<name>` followed by a byte that is neither `/` nor a name byte, or the end of the text, and rewrite it to `~`. Touches `W-HOME-PATH`, the archive rewrite and the upstream scrub at once; flip the `/home/x and` assertion in `paths.rs` tests.
