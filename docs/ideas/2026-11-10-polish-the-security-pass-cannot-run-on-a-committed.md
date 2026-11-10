---
title: "/polish: the security pass cannot run on a committed branch, and the UX pass shares the test database"
state: "open"
added: "2026-11-10"
origin: "factory upstream"
---

Two steps of `/polish` §1, as written, do not work in a `factory` run's lane. Both were reported in three of seven feature retros in one project.

## 1. The security pass names a skill that only reads uncommitted changes

`/polish` asks for the security pass by running the `security-review` skill as a subagent. That skill reviews pending (uncommitted) changes. By polish time every slice is committed on the feature branch, so the skill finds nothing to review. In each case the agent noticed and fell back to a manual review of `main...HEAD`, but the skill text never says to.

**Should.** Tell the security pass to review the branch diff (`git diff main...HEAD`), or pass that range to the security skill explicitly. Say what to do when the skill cannot take a range.

## 2. The UX pass and the test runner share one database in a lane

A lane worktree points its app and its test suite at the same database (the lane's `DB_DATABASE`; the project's separate test database file is not seeded into lanes). The UX pass drives the served app while the polish agent runs scoped tests. A test's database refresh wiped the data under a live UX drive twice; once the UX agent moved to a scratch database to finish, which other rules forbid. Another time, polish simply waited for the UX pass before testing.

**Should.** Order the steps: run the UX pass before any fix that needs tests, and run no tests while it drives. Or give the UX pass its own lane or database, and say which one the served app reads. The lanes skill could record whether a lane's app and tests share a database, so polish knows which ordering applies.

## Minimal shape

```
polish §1: security → security-review skill → "no pending changes" → manual fallback (undocumented)
polish §1: UX drive on lane DB ‖ pest --filter … (RefreshDatabase on lane DB) → drive data gone
```
