# Engineering Decisions

Standing calls that outlive the feature that raised them. Written during `/polish`, when a review finding needed a design decision that could plausibly come up again.

**Read this before escalating a review finding.** An exact match means the question is already answered — apply it and cite it, do not ask again. A near match means ask, but lead with this as the recommended option.

**Rules**
- **An entry is `## {Short name}`**, then **Chose**, **Over**, **Because** and **Applies when**. No number, date or origin. Cite it by its heading's anchor: `decisions.md#who-talks-to-github`.
- **Old links keep resolving.** A renamed or merged entry keeps every old anchor as an `<a id="…"></a>` line above its heading, the old `dec-n` included.
- **A reversed or merged loser folds into the winner's Over**, with the reason it lost. Its heading goes; its anchors move above the winner's. An Over is never lost.
- **Most decisions do not belong here.** An entry earns its place only if it could recur, was not obvious, and has a real **Applies when** trigger. If the trigger is one function in one file, it is a fix, not a decision. Noise makes this unsearchable, which defeats the point.
- Not a style guide and not a rules file — those are `CLAUDE.md` / `AGENTS.md` / `.ai/guidelines/`. This is a record of forks taken, with the road not travelled. A domain fact goes in the doc that owns its topic.

---

<a id="dec-1"></a>
## How does the driver commit?

**Chose:** every driver commit names its paths (`git commit -- <paths>`) and checks that HEAD is on the run's branch first. Off the branch, the driver parks or blocks; it never commits.
**Over:** a bare `git commit` that trusts HEAD and the index. Workers share one checkout, so a bare commit sweeps in a sibling's half-staged files, and a worker can move HEAD to another branch.
**Because:** both have happened. A commit is the record a resumed run trusts, so the wrong paths or the wrong branch corrupt it silently.
**Applies when:** adding any driver path that commits or pushes.

<a id="dec-2"></a>
## Who talks to GitHub?

**Chose:** workers never do. The driver makes every `gh` call and every push. A worker gets `gh` and `git push` denied whole, no GitHub token, an empty `gh` config dir, and a git push-off env.
**Over:** a denylist of the dangerous `gh` forms (`gh pr merge`, `git push --force`). That set is open-ended: REST and GraphQL calls, wrappers, aliases. Denying the whole tool and withholding the credential is a closed set.
**Because:** the never-list is held against an agent under pressure. The driver is the one place that can check the branch, the caps and the PR state before acting, and a worker cannot.
**Applies when:** adding a new worker kind, or a contract asset or prompt that might ask a worker to touch GitHub.

<a id="dec-3"></a>
## How are driver state files written?

**Chose:** atomically (a temp file in the same directory, then a rename), and under a lock where several processes write. A reader that meets a torn or unparseable record skips it and reports it; it never fails the run.
**Over:** a plain `fs::write`, which lets a reader in another process see a half-written file.
**Because:** the driver, `factory stop` and `factory answer` are separate processes racing on the same files. One torn answer file once stopped every slice of a run, and the test stub failed CI on the same race.
**Applies when:** adding any file the driver writes and later reads back.
