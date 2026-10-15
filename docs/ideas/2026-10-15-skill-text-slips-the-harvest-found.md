---
title: "Skill text slips the harvest found"
state: "open"
added: "2026-10-15"
origin: "harvest 2026-10-15"
---

Five line-level fixes. Each was a guess a worker had to make, and they ship together in one skill bump.

- **polish §1 and §2, leak hits** (PRD-007). §1 says "`n fix hits`" while one hit prints `1 fix hit`, and §2 never says to read the `## Leaks` part's hits. Fix: "`n fix hit(s)`" in §1, and at the top of §2: "A leaks verdict with hits → read the `## Leaks` part too: each hit sits under its fix line." Folds the idea `2026-10-15-polish-2-should-name-the-leaks-hits-as-something-t`.
- **polish §8, cross-slice fixes** (PRD-005, PRD-006). "The slice's Code-review fixes section" assumes one slice. Fix: each fix goes to the slice that owns the code it changes.
- **polish §9 in a factory run** (PRD-007). The contract replaces every closeout step. Fix: one line at the top of §9, "In a factory run, skip to Report."
- **prune §5, names the log quotes** (PRD-004). It protects test names in evidence and Progress lines, not in a log entry's Undo line. Fix: extend the rule to the feature's `log.jsonl`.
- **/slice, a file two FRs both write** (PRD-008). FR-5 and FR-6 both changed `docs/index.md`, so the slice built one half and the QA guide claimed both. Fix: /slice flags a file two FRs write, and puts both edits in one slice or names their order.
