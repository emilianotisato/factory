---
title: "polish: scale the second-opinion fallback instead of one agent per pass"
state: "open"
added: "2026-11-20"
origin: "factory upstream"
---

**What happened.** When `factory review collect` reports `fallback owed`, `/polish` §1 asks for one fresh agent per pass of about 600 changed lines, the same passes the vendor would have run. On large feature branches that meant 13 to 30 agents. Several passes held only docs, a lockfile or test files. In six features in a row, the polish worker had to guess: it grouped passes (7 or 9 agents of about 1,100 to 2,600 lines each) and skipped docs, lockfile and test-only passes. Those grouped agents still found most of the medium defects.

**What it should do.**
- Cap the fallback (for example, at most N agents, N in the plan line) and group passes up to the cap.
- Say which passes may go without a fallback agent: lockfiles always; test-only passes when the local review covered them.
- Keep a pass for prose a user will read (a guide sent to a customer). One run found a guide promising a behaviour the code did not have.
- State the choice in the plan line, so the PR body can say what had no second reader.

**Minimal shape.** A 15,000-line diff splits into 30 passes: 9 code, 6 frontend, 10 tests, 4 docs, 1 lockfile. The fallback becomes "8 agents over code and frontend, 1 over user-facing docs; tests and lockfile left to the local review".
