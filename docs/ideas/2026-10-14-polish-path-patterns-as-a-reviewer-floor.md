---
title: "Polish path patterns as a reviewer floor"
state: "open"
added: "2026-10-14"
origin: "PRD-007 interview"
retrigger: "a polish skips a security or UX pass a change needed"
---

Cut from PRD-007 at the cut (2026-10-14), with no evidence yet: no polish on record skipped a pass it needed. The shape the interview settled before the cut, so a future PRD does not redo it:

- **`[polish]` in `docs/project.toml`**, `security = [...]` and `ui = [...]`, gitignore-style globs matched with the `ignore` crate already in `Cargo.toml`. A touched path that matches makes the security or UX pass required. A bad pattern errors naming the entry, like `[leaks] allow` (`src/leaks.rs:408-430`). Written by the owner by hand.
- **The floor is binding by contract plus a driver check.** The polish contract gains `REVIEWERS: local, security, ux, second`; after a `done` worker the driver writes one `decide` decision per required pass missing from it, through `log::add` (`src/docs/log.rs:195`). Never blocks or respawns.
- **Discovery.** A `polish` line in `factory docs doctor` beside `skills` and `leaks` (`src/docs/doctor.rs:65-75`); with no table, the polish worker writes a ready `[polish]` block of the paths it judged security or UI into the retro's Project friction; a bullet in `docs/reference/adopt-a-repo.md` phase 4, because `/prd` preflight does not walk doctor lines (`skills/prd/SKILL.md:22-28`).
- **Classes.** Security and UI only. A blast class (migrations, money, public API forcing every reviewer) was rejected.

PRD-007's `factory review collect` writes the plan section these would extend.
