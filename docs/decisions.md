# Engineering Decisions

Standing calls that outlive the feature that raised them. Written during `/polish`, when a review finding needed a design decision that could plausibly come up again.

**Read this before escalating a review finding.** An exact match means the question is already answered — apply it and cite it, do not ask again. A near match means ask, but lead with this as the recommended option.

**Rules**
- `DEC-n` is monotonic and never reused.
- A reversed decision is **superseded, not deleted** — leave the entry and add `**Superseded by DEC-n.**`
- **Most decisions do not belong here.** An entry earns its place only if it could recur, was not obvious, and has a real **Applies when** trigger. If the trigger is one function in one file, it is a fix, not a decision. Noise makes this unsearchable, which defeats the point.
- Not a style guide and not a rules file — those are `CLAUDE.md` / `AGENTS.md` / `.ai/guidelines/`. This is a record of forks taken, with the road not travelled.

---

### DEC-1 — How does the driver commit? · 2026-10-11 · PRD-001 polish

**Chose:** every driver commit names its paths (`git commit -- <paths>`) and checks that HEAD is on the run's branch first. Off the branch, the driver parks or blocks; it never commits.
**Over:** a bare `git commit` that trusts HEAD and the index. Workers share one checkout, so a bare commit sweeps in a sibling's half-staged files, and a worker can move HEAD to another branch.
**Because:** both happened in PRD-001's run (L3, L12, AD-20005, AD-20007, AD-30009). A commit is the record a resumed run trusts, so the wrong paths or the wrong branch corrupt it silently.
**Applies when:** adding any driver path that commits or pushes.

### DEC-2 — Who talks to GitHub? · 2026-10-11 · PRD-001 polish

**Chose:** workers never do. The driver makes every `gh` call and every push. A worker gets `gh` and `git push` denied whole, no GitHub token, an empty `gh` config dir, and a git push-off env (AD-10001).
**Over:** a denylist of the dangerous `gh` forms (`gh pr merge`, `git push --force`). That set is open-ended: REST and GraphQL calls, wrappers, aliases. Denying the whole tool and withholding the credential is a closed set.
**Because:** the never-list is held against an agent under pressure. The driver is the one place that can check the branch, the caps and the PR state before acting, and a worker cannot.
**Applies when:** adding a new worker kind, or a contract asset or prompt that might ask a worker to touch GitHub.

### DEC-3 — How are driver state files written? · 2026-10-11 · PRD-001 polish

**Chose:** atomically (a temp file in the same directory, then a rename), and under a lock where several processes write. A reader that meets a torn or unparseable record skips it and reports it; it never fails the run.
**Over:** a plain `fs::write`, which lets a reader in another process see a half-written file.
**Because:** the driver, `factory stop` and `factory answer` are separate processes racing on the same files (L10, L11, AD-20012). One torn answer file once stopped every slice of the run, and CI round 1 failed on the same race in the test stub.
**Applies when:** adding any file the driver writes and later reads back.
