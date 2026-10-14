---
title: "leaks: allowlist trust, scan cost, tuning candidates"
state: "open"
added: "2026-10-14"
origin: "PRD-006 polish"
---

Three trust and cost gaps in `factory leaks`, found by the PRD-006 polish security pass and the fallback second-opinion agents.

- **The allowlist is read from the worktree being scanned.** A checkpoint worker can add `[leaks] allow` in `docs/project.toml` and its own hits vanish from the next `# Leaks` section. The Autonomy contract forbids it, but nothing checks it. Option: read the allowlist from the base ref (`git show <base>:docs/project.toml`) when `--base` is given, and print allowlist changes on the branch as their own section.
- **Cost.** `docs doctor` and `docs preflight` run `--all` over the whole tree each time (3.5 s on cody-agent, 4.2 s on arca-api with a debug build). There is no cap on file size or count; lockfiles and large json/yaml are read and count in the comment-share denominator. Option: skip lockfiles and files over a size cap, keep data formats out of the share, and cache the `--all` verdict by tree hash.
- **Tuning candidates seen in the field but not fixed:** `C99`/`C11` (language standards), "task 1" in prose, `block1`/`cp1` as Bézier names, `Dec-2026` dates. Each needs a non-hit unit test before a pattern change.
