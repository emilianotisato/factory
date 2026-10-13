# Skill family decisions, 2026-08 to 2026-10 (imported)

Imported 2026-10-13 from the Dropbox skills repo `feature-docs/MAINTAINING.md` when `feature-docs` was eliminated. History only: new decisions about a skill go to `docs/decisions.md` as `DEC-n`, and the reasoning behind a binary change goes in its PRD. The standing instructions still hold where they do not name a v2 mechanism.

---

# Maintaining this framework

**Read this before changing anything in `feature-docs/`, `prd/`, `prd-to-tasks/`, or any other skill in this family — `deliver/`, `polish/`, `verify-app/`, `ux-review/`, `guardrails/`, `test-cleanup/`.** It holds the project owner's standing instructions and the reasoning behind numbers that would otherwise look arbitrary. It is not user-facing documentation — it is the contract for whoever edits the skill next.

Add to the **Decisions** log at the bottom whenever a call is made that a future agent would otherwise re-litigate or silently reverse.

---

## Standing instructions

**Version bumps are rare and structural.** `FRAMEWORK_VERSION` goes up only for a change that makes existing docs *invalid* — a new required field, a moved file, a renamed convention — **or when the owner explicitly says to bump it.** Never for:

- tuning a threshold or a default
- rewording, restructuring or trimming a template
- adding a command, a lint rule, or new guidance
- redefining what a status *means*, as long as no existing doc becomes wrong

Version churn is how upgrades stop happening. When in doubt, do not bump — a project that is silently current is better than one nagged into a no-op migration. Every real bump ships its `upgrades/vN.md` in the same change; a version with no guide is not a version.

**The PRD is a build document.** The audience is the owner and the coding agent, in that order. No personas, no stakeholder tables, no success-metrics dashboards, no rollout-phase theatre. If a section cannot change a line of code, it is context the agent pays for and gets nothing back. A human-facing export (client, PM, board) is a separate artifact generated on request — roughly 1% of cases, and cheap to produce *from* a good build PRD. The reverse is not true, which is why the default leans this way.

**Interview with the AskUserQuestion tool.** It is the primary mechanism during `/prd`, not a fallback for tidy questions. Every decision fork gets 2-4 concrete options with their costs, recommendation first. Prose questions only for genuinely unbounded answers.

**Defaults must be measured, not invented.** If a number goes into this framework, the reasoning goes in the Decisions log below with the data behind it. "Seemed about right" is how the 500-line ceiling ended up flagging healthy documents.

**Never invent history.** Migration, upgrade and repair all move and annotate; they never fabricate a status, a date, or an FR range. Anything inferred is reported back in one list at the end.

**Run every command a skill tells an agent to run, before shipping the skill.** Not a sample — each one. A path that does not exist, an env var that is only set inside a plugin's own context, a flag that moved between majors: all of these read as correct prose and fail on first use, in someone else's session, with no obvious cause. This is the cheapest bug class in the family to prevent and the most expensive to diagnose.

**Learn from every real run, the same day.** A skill's first run against a live repo is worth more than any amount of re-reading it, and the lessons decay fast. So after one:

1. **Read the report for what the skill should have known**, not for whether it succeeded. A run that produced a good result and cost an hour on one surprise has one finding, not zero.
2. **Fix the skill immediately**, while the detail is recoverable. A correction deferred is a correction reconstructed from memory, badly.
3. **Route each learning to exactly one place** (below), then say what changed.

Three runs on one repo produced fourteen corrections this way. Treat a first run that produces none as a sign the report was not read closely enough.

### Where a learning lands — one place, never two

**The rule goes in the skill body. Only the reasoning comes here.** Writing both is how this file doubled in a day, and it is why a reader cannot tell which half is authoritative.

- **A rule an agent must follow** → the skill body, in the step where it applies. Nothing else.
- **A stack-specific command or trap** → that skill's `references/`.
- **The reasoning behind a call a future agent would re-litigate or silently reverse** → an entry here.

An entry earns its place on the same three tests `docs/decisions.md` uses on itself. Could the question recur. Was the answer non-obvious. Can you state the trigger without naming one file. **Most learnings fail all three and belong only in the skill.** Compress an entry to the argument; if it restates what the skill already says, delete that part. A log nobody can read through is a log nobody checks before reversing a decision, which is its only job.

---

## Decisions

### 2026-08-13 — Git: commit freely, attribute solely to the owner

The friction was never a config rule — `permissions.deny` was empty. It was the harness default ("commit or push only when the user asks") plus git not being allow-listed, so every call prompted and agents reported work as "uncommitted as per your request".

Split two ways. **Global** (`~/.claude/CLAUDE.md`, loads everywhere): commits allowed without asking, no `Co-Authored-By` / "Generated with" / session-link trailer ever, commit messages via the `caveman-commit` skill, branch creation when obvious. **Framework-scoped** (`feature-docs/SKILL.md`): *where* the commits go — PRD, slices, one per slice, polish fixes, closeout.

Permissions were widened to the safe subset only: status, diff, log, show, `branch --list`, rev-parse, ls-files, add, commit, mv, `checkout -b`, `switch -c`. **Deliberately excluded**: `push` (outward-facing), `reset`, `rebase`, `clean`, and bare `checkout` (destroys uncommitted work). Widen only if the owner asks.

`attribution: {commit:"", pr:"", sessionUrl:false}` **replaces** the deprecated `includeCoAuthoredBy` (it is not a second layer — see the correction below).

### 2026-08-17 — Attribution is enforced by a hook, not by settings

Settings alone did not hold. Between 2026-08-14 and 08-16, three squash-merges on a project landed carrying `Co-Authored-By: Claude …`, which the owner has never wanted.

Cause: `includeCoAuthoredBy` was deprecated out of the code path *before* the build running that week honored `attribution`, so for a few days nothing suppressed the commit trailer. The tell is which trailer flipped — `sessionUrl: false` (a boolean) took effect immediately, while `commit: ""` did not land until the newer CLI. The schema, read out of the 2.1.224 binary, is:

```
attribution.commit     string  — "Empty string hides attribution"
attribution.pr         string  — "Empty string hides attribution"
attribution.sessionUrl boolean — default true
includeCoAuthoredBy    boolean — "Deprecated: Use attribution instead"
```

**The deeper reason settings can never be the guarantee:** attribution is applied at the *prompt* level. `includeGitInstructions` (default true) puts commit and PR rules in the agent's system prompt, and `attribution` only edits what those rules say. A stale build, a cloud session that never reads local settings, or a different agent entirely can still type the trailer by hand.

So the enforcement is a global `commit-msg` hook at `~/.config/git/hooks/commit-msg` (the owner has `core.hooksPath` set globally, so it covers every repo). It strips `Co-Authored-By` / `Assisted-By` / `Generated-By` trailers naming an AI tool, the `Claude-Session:` trailer, and "Generated with …" lines. **Human co-author trailers are deliberately preserved** — the pattern matches the tool name, not the trailer key. Settings stay as the polite request; the hook is what makes it true.

### 2026-08-13 — `/polish` is a skill; the binary only nudges

The pre-merge ritual (local review + `coderabbit review --agent`, dedup, triage, fix, `test-cleanup pr`, closeout) is judgment work, so it lives in the `polish` skill. The binary's whole contribution is an optional `reviewed:` date and two **chained** warnings — `W-NOT-REVIEWED` then `W-READY-TO-SHIP` — so only one nudge fires at a time, both at commit time through the existing hook. `docs ship` says so loudly when `reviewed:` is missing but **never refuses**; a block would turn a helpful nudge into a fight the one time there is a good reason to skip.

A git hook was rejected for the ritual itself: it blocks, cannot ask a question, and cannot resume.

**Ordering is load-bearing and documented as such.** `test-cleanup` runs *last* so the fix pass can add regression tests freely; both reviewers are collected and deduped *before* any fixing, so nothing is considered or fixed twice. A future agent will want to "optimise" both — the reasoning is written into the skill to stop that.

**The gate was inverted.** The old ritual asked "shall I fix these?" and the answer was always yes, which makes it friction rather than safety. The Fix bucket is now applied without asking; the interruption is reserved for findings whose *remedy is a genuine choice* — the test being "does the finding determine its fix?" — plus anything contradicting a settled `OPEN-n` or `DEC-n`, which is re-litigation rather than a bug.

### 2026-08-13 — `docs/decisions.md`, and why most decisions do not go in it

Escalated review decisions recur across features, so they cannot live in the PRD that raised them — three PRs later that PRD is in a frozen `archive/` folder nobody reads for guidance. `decisions.md` sits beside `ideas.md`: **ideas = not yet decided to build, decisions = decided and still applying.** `DEC-n` monotonic, superseded-not-deleted.

The payoff is at triage time, not writing time: `/polish` reads it before escalating, so an exact match is auto-applied with a citation (never asked twice) and a near match is escalated with the prior decision pre-loaded as the recommended option.

**Owner's constraint: not every escalated finding earns an entry.** A bloated log is unsearchable, which defeats the only reason it exists. Three tests — could it recur, was it non-obvious, and can you write a real **Applies when** trigger. The third is the sharpest: if the trigger names one function in one file, it is a fix, not a decision. The agent **recommends with reasoning and the user chooses**; the default is not to log.

Both `decisions.md` and `reviewed:` are optional additions — nothing existing became invalid, so **no version bump**.

### 2026-08-13 — Size ceilings are in KB, not lines

Lines are a bad proxy: these documents run **~115 bytes/line** (long prose paragraphs, wide tables), so a line count says nothing about what an agent pays to read one.

Measured across a real tree using this framework:

| Doc | Lines | Bytes |
|---|---|---|
| `archive/prd-v1.md` — the one the framework itself calls a mistake | 1188 | 149KB |
| `012-admin-console/prd.md` — largest healthy live PRD | 612 | 72KB |
| `013-admin-ux-ia/prd.md` | 298 | 33KB |
| largest slice | 113 | ~12KB |

The old 500-*line* ceiling flagged 012 (healthy) and would have missed a 1000-line doc at 60KB. Replaced with **120KB for a PRD, 30KB for a slice** — comfortably above the owner's stated ~800-line norm, so the only thing that fires is a document already agreed to be too big. Overridable per project via `prd_max_kb` / `slice_max_kb` in `docs/index.md` frontmatter.

### 2026-08-13 — `shipped` is a claim the branch makes, not an observation about production

Defining it as "merged and live" made it unknowable from inside the PR, so it always arrived as a stray bookkeeping commit on the *next* feature's branch. Redefined as "every slice done and this branch completes the feature", set in the last commit of its PR. Because the docs live in the same branch as the code, the status flips on `main` exactly when the merge lands — there is no window where `main` lies. `docs ship` and `docs archive` both refuse while a slice is open. **No version bump**: no existing doc became invalid.

### 2026-08-13 — Frontmatter, not a central YAML file

Considered a single `docs/state.yaml`. Rejected: it is a second status board (violates invariant #1), it cannot replace `index.md` because the Notes cells are hand-written prose, it needs manual GC when folders move, and it produces one merge conflict per concurrent feature. Per-file frontmatter has locality (state travels with the file when archived) and never conflicts across parallel worktrees. Capped at five keys for a PRD (plus the optional `reviewed:`) and three for a slice — the moment scope or notes move into frontmatter, the document stops being the document.

### 2026-08-13 — `archive/` is grandfathered forever

An early version of `check` demanded frontmatter on archived PRDs, which ordered the user to violate the framework's own frozen-archive rule. Version requirements apply to `features/` only. Same for size warnings and broken relative links: a link that went stale when a folder moved into `archive/` is the rule working, not a defect.

### 2026-08-13 — The tool never does judgment work

`docs` decides only what a machine can: numbering, links, statuses, dependencies, frozen archive, conformance gaps. Inferring a PRD's status or FR range from prose is the agent's job, and it must be proposed before it is applied. `doctor` exists to split a gap into *mechanical* and *judgment* precisely so this line stays visible.

### 2026-09-05 — The verify gate is declared once, in `index.md` frontmatter

Every skill that needed the gate hunted for it in `CLAUDE.md` / `AGENTS.md` / `.ai/guidelines/`, and nothing noticed when a project had none — the owner recalled it per repo, from memory.

**One string, always.** A polyglot repo wraps its several gates in one target rather than the framework storing a list; building that wrapper is `/guardrails`, because the right gate differs per stack. **The tool resolves the entry point and never runs the command** — a real gate takes minutes, and running it is the agent's job at Verify time. **Warn, never refuse**, for the same reason `docs ship` never refuses: a block turns a nudge into a fight the one time there is a good reason to skip. Optional field, no version bump.

### 2026-09-05 — §QA plan is mandatory in a `ready` PRD

Agents had no standing instruction to prove a change on the real surface; the Verify task ran the test gate and stopped. Borrowed from pstack's `create-verification-skill`: proof means driving the user path, not "tests green".

**Enforced by a heading regex, deliberately, not a frontmatter flag** — a flag lets a PRD claim a plan it does not have, and the point is the table's content. Draft is exempt because it is still being interviewed, archive because it is frozen.

**A missing harness is a decided prerequisite with a stated cost, not an OPEN.** That is what makes it land as a slice and get built once per repo instead of re-debated per feature. No version bump.

### 2026-09-05 — `/verify-app`, and why the driver is pushed rather than offered

§QA plan made "harness: none" a legal line with nothing behind it. `/verify-app` is what the harness slice runs.

**Owner's rule: be pushy, not soft.** A missing driver is proposed as the recommended option with commands and cost, including a test-framework major upgrade when that is the blocker. One refusal is honoured; then the fallback is taken and the generated skill states which proof standard was traded away. The reasoning: the harness is a dependency of proof, and a soft offer is how a repo drifts back to manual QA.

**Two drivers, always named apart.** The scripted one is repeatable and lands in the gate; the interactive one is the agent's hands and is never a substitute. A proof that used only the interactive driver says so.

### 2026-09-06 — `/guardrails`: measured floors, proven to fail

The interview the owner had been running from memory on every new repo. Question bank: cody-agent's PRD-011, generalised across six stacks.

**Measure before proposing.** A floor above what the code already does is red on day one and teaches everyone to bypass the gate. **Floors ratchet up only**, and that rule goes in the *project's* verification doc rather than only the skill, because an agent under pressure lowers a floor to get green and a ratcheting-down floor is indistinguishable from no floor.

**A gate nobody has seen fail is a decoration**, hence the sacrificial-violation pass. **Incremental test selection never enters the gate** — adding it looks like a free speed win and quietly guts it. **Escape hatches are mandatory on slow tiers**, because the alternative is someone deleting the tier permanently and silently. **Local-versus-CI is asked, never assumed**, and the answer is documented, or someone later "fixes" the divergence by accident.

### 2026-09-06 — Taste is mostly not taste

The premise behind `/ux-review`: most of what the owner fixed by hand was *absence* — no empty state, an error with no way out, a stripped focus ring, spacing from nowhere. Absence is checkable by driving the app, so it should never reach a human.

**Every finding carries a number**, because a measurement needs no argument. **A state that could not be reached is reported unreached**, never as passing.

`docs/ux-rubric.md` is the third optional standing file: **not yet decided to build / decided and still applying / what finished looks like on screen.** Written out of the code, with the best existing example cited by path as the reference implementation; the owner is asked only where the codebase contradicts itself, with call sites counted. A rubric derived from a style guide is noise.

**The loop is the point.** A correction becomes a rubric line; a rubric line a machine could check becomes a lint rule or a gate tier *and leaves the rubric*. The intended end state is a short rubric and a long enforced-mechanically list. `/ux-review` is a **collector** inside `/polish`, not a fourth ritual — one triage, one question round.

### 2026-09-06 — Model routing: roles in one file, bars in prose

pstack routes subagents through a config mapping some twenty roles to model slugs, written by a setup interview. **The role indirection is durable; the hardcoded slugs are not** — pstack needs a mid-run recovery path for a rejected slug, which is the tell. So `models.md` holds four roles and is the only file in the family naming a model. No setup interview: four roles across six skills does not earn one.

**A bar is prose, fires only when the session is under it, and is never a question.** A skill cannot change the session model, so telling the user is the entire contribution, and a note that fires every run is ignored within a week. **No bar at all where the model does not change the outcome.**

**A precondition is not a bar.** `/ux-review` without vision fails *silently*, which is worse than failing loudly, so it degrades in a stated way and names the half nobody looked at.

**`/prd`'s adversarial pass is delegated** — an author defends its own reasoning. Never a `fork`, which would inherit the reasoning being attacked. The parent still owns the verdict, because a reviewer without the interview in context re-litigates settled decisions.

**Cross-vendor is the whole point of the second-opinion role.** Two Claude models agreeing is correlation, not evidence.

### 2026-09-06 — What the first real runs changed

Three runs on a live repo (`/verify-app`, `/guardrails audit`, `/ux-review`), all corrected the same day. The corrections live in the skills; the reasoning worth keeping is here.

- **One skill per *driver*, not per surface.** Two surfaces one browser reaches share one Launch, Doctor and Cleanup. Splitting triplicates the lifecycle and it drifts within a month.
- **Probe for what defeats the obvious assertion before writing a map against it.** A shadow root makes text assertions fail while the text is plainly on screen, which reads as a broken app rather than as the wrong tool.
- **Stale builds are the worst failure mode in this family.** A gate that tests without building drives whatever is on disk, and a green result proving nothing is indistinguishable from a real one. Found twice in two days from opposite directions — by the harness (it drives a stale bundle) and by the audit (the gate never builds one) — which is what turned it into a standing check in both.
- **Gates decay by neglect, not violation.** Nobody lowers a floor; the code improves and the floor sits still. Hence headroom in every audit, and thresholds living in exactly one place, because duplicated numbers drift and agents believe the prose.
- **Mutation structurally resists being proved red** — a healthy suite kills mutants redundantly and a floor cannot exceed 100%. Prove it with a new assertion-free sacrificial class, or prove only the mechanics and record it *unverified, not disproven*.
- **Evidence must survive an unrelated later run**, not only its own cleanup.

### 2026-09-06 — Three calls made while fixing the framework's own drift

**Foundations are reported, never enforced.** `doctor` now shows gate, harness and rubric; `check` warns on none of them beyond the gate. A library with no user surface correctly has no harness and a project with no UI correctly has no rubric, so a warning would cry wolf at projects that are already right. The order they are built in is not a preference: the harness slice needs a gate to Verify against, and the rubric is only checkable by driving the harness.

**`slice-00` was killed the day it was introduced.** Eleven references across five files named a `slice-00-harness` that `docs slice` could never create, since it numbers from one. Teaching the tool a `--number` flag was the alternative; the convention lost instead, because slice numbers are feature-local and identity-free, so "the harness slice goes first" carries the same meaning with no machinery. **The lesson is older than the bug**: prose that names an artifact the tool cannot produce reads as correct until someone runs it.

**`/polish` scales its reviewers to the diff.** Four reviewers on a one-line fix is theatre, and — the real risk — a ritual that always costs an hour gets skipped entirely, losing the cheap reviewers along with the expensive ones. Two rules override the table: anything user-visible gets the UX pass however small, since a one-line CSS change is exactly where a state breaks; and CodeRabbit is skipped on trivial diffs, where it spends the owner's quota at its worst signal-to-noise.

### 2026-09-10 — Cursor is the second opinion, and presence is not a signal

The Codex subscription was paused. The plugin and CLI **stay installed** and are expected back, which is exactly why the old phrasing had to go: every skill said "cross-vendor when the Codex plugin is installed", so a probe would find the binary, take the Codex path, and fail on billing. **Installed does not mean usable.** The rule now is unconditional — Cursor CLI is the second opinion, no probe, no branch — and switching back is a deliberate edit to `models.md` on the owner's word, not a runtime detection.

The Codex mirror in `self-learning` was deliberately **kept**. It costs one write and the rule should already be there the day the subscription returns.

**Three ways the Cursor invocation fails silently**, all found by running it rather than reading about it:

- **`--trust` is required.** Without it the run exits in under a second with a workspace-trust prompt and no findings. An agent that does not check the exit reports a clean review. This is the worst failure mode in the family — a reviewer that confidently found nothing because it never ran.
- **`--output-format text` buffers to completion.** Zero bytes until the end, so a long review and a hang are indistinguishable. The cost of this was one real session: eight minutes watching a process that was working fine. `stream-json` to a log file, read with `jq`, never raw into context — the stream repeats the full argument blob on every tool event.
- **The docs' own claim that `--mode ask` refuses shell was wrong.** It refuses edits and allows read-only shell, which is what lets the reviewer run `git diff` itself. Believing otherwise means pasting a diff into the prompt and paying for it twice.

**Scope is the reviewer's job, not the context window's.** The failing run asked one pass over 9,411 lines across 83 files. It fit; it was still the wrong ask. Past ~30 files or ~2000 lines, name the risky subset. A model with a million-token window will happily skim.

**CodeRabbit's free tier changes which limit matters.** Not cost any more — **150 files per CLI review** (after path-filter exclusions) and **3 CLI reviews per hour**, from the vendor's own plans table. Two consequences: over 150 files a review is truncated, and a truncated review reported as complete is worse than a skipped one; and free-plan *PR* reviews are summary-only, so the GitHub bot is no longer a fallback for the CLI. This **supersedes the quota clause** in the 2026-09-06 entry — the trivial-diff skip survives, but the reason is noise, not spend.

**Cursor needed almost no work in `self-learning`.** Its CLI already reads `<repo>/CLAUDE.md` and `<repo>/AGENTS.md`, so every project-scoped write reaches it for free. The two gaps are honest dead ends: global User Rules live in an account-side UI pane with no file path, and plugins install from git marketplaces rather than a writable directory. Both are reported to the user as a manual step. **Inventing a plausible path would have been the failure here** — a rule written where nothing reads it is worse than one the user knows is missing.

### 2026-09-16 — Paper cuts, fixed before the framework runs unattended

Contradictions a human reader smooths over and an agent running alone follows literally. Most fixes live only in the skills; two calls are worth keeping here.

**A command owned by one file is pointed at, never copied.** `/polish` carried its own copy of the Cursor invocation, and the copy kept `gpt-5.6-sol-xhigh` after `models.md` moved the default to Grok *because* that model was rate-limited — so polish would have died mid-run on the exact failure the default change existed to avoid. The copy is always the one that drifts. Skills now reference `models.md`'s invocation and substitute paths.

**Slice ceiling: ~10 tasks, Verify and Prove included.** `prd-to-tasks` said ~15 and its own template said ~8. Measured across 362 real slices, 274 with numbered tasks: median 6, p90 8, fifteen above 8, three above 9 (11, 13, and an 18 that is a migrated task list). 8 would flag healthy slices now that every slice carries two mandatory closing tasks; 15 catches only the migration artifact. 10 sits just above the healthy tail.

The index template's "When done" still called `shipped` "merged and live", three sections below its own redefinition. Wording only, no bump — and nine existing projects' `docs/index.md` still carry the old line, since a template change never reaches docs a project already owns.

### 2026-09-16 — `/deliver`: an approved PRD to a green pull request, unattended

The owner answered "continue with the next slice?" with "yes, of course" every time. A gate that always gets the same answer is friction — the argument that inverted `/polish`'s Fix bucket. `/deliver` runs from the approved PRD to a draft PR with CI green. The merge stays the owner's.

**Running it is the approval.** A `draft` PRD flips to `ready` only when `docs preflight` passes; there is no separate approve step.

**One mode, and only one documented.** A blocker stops the run and notifies. The owner expects to want `--park` (block one slice, finish the independent ones) and `--autonomous` later. They are recorded here and **in no skill** until built — the `slice-00` lesson: prose naming what does not exist reads as true. Sub-skills only *classify* a blocker and `/deliver` alone applies the mode, so a new mode is an edit to one file.

**One contract, pointed at.** Every skill already had its own ask-the-owner moments. Rather than an unattended variant of each, `autonomy.md` holds one test — does the PRD or a DEC settle it? decide; else reversible? log; else block — and each touchpoint carries a one-line `Unattended (/deliver):` marker. Greppable, and a rule change is one edit.

**The owner's standing files are never written unattended** — `decisions.md`, `ux-rubric.md`, gate config, CI workflows, `CLAUDE.md`/`AGENTS.md`. Proposals go in the PR body. An agent alone under pressure lowers floors and adds suppressions; the retro is the sanctioned place to call a rule too strict, which is half its reason to exist.

**The retro pairs cost with catch.** Asked only about friction, an agent votes to cut every check, because every check costs it time. A cut or tune verdict must name what the step failed to catch. Five numbers sit in frontmatter for a later `docs retro --scan`, **deferred until three real retros exist** — the format will move after the first runs.

**Ship, never archive.** Archiving freezes the folder, and the global pre-commit hook would then reject the owner's review notes on `agent-log.md`.

**A small PRD gets one slice under `/deliver`.** Attended, it builds straight from `prd.md`. Unattended, the slice file is where progress, blockers and the resume point live.

**No docs-only push after the PR opens**, because each one restarts CI. So the retro is written in the closeout commit, CI fix commits bump `ci_cycles` in the same commit, the index Notes say "PR from `feat/{slug}`" rather than a number, and a flaky rerun is noted in the PR body instead of the log.

**Enforcement is not prose** — the 2026-08-17 lesson again. `deny` rules hold in bypass mode (documented) and match each part of a compound command; `gh pr merge` is an API call no local hook can see, so the deny rule is its guard. A global `pre-push` hook refuses default-branch pushes, deletes and non-fast-forwards, **only when `CLAUDECODE=1`**, so the owner's own pushes are untouched.

**The tool stays out of judgment.** `preflight` refuses only on what a machine decides and *lists* unsettled OPENs. Measured across every live PRD in `~/Projects`: OPENs get settled two ways, strikethrough or a bold verdict (`**Closed 2026-09-11:**`, `**CERRADA**`). A first regex that knew only strikethrough missed a genuinely open `OPEN-5` in arca-api's PRD-012. With both recognised, every OPEN it flags in that corpus is really open or deliberately parked.

**The caps are guesses** — 3 gate attempts, 3 CI rounds, 1 flaky rerun — and `autonomy.md` says so. `gate_runs`, `gate_red` and `ci_cycles` in the retros are the data to replace them.

**A docs page was wrong about this harness.** It says a background Bash command does not wake the agent on exit; running one showed it does. The CI watch relies on that — tested, not read.

No version bump: new optional files, a new slice status value and new commands leave every existing doc valid.

### 2026-09-16 — `pr-review` folded into `/polish`, then deleted

An older standalone skill: three parallel lenses (security, optimization, test coverage) over a GitHub PR, with severity tables. Each part was checked against what the family already does.

**Kept — the security lens.** `/polish` had none: its blast-radius row named auth and money, then sent them to reviewers with no security focus. It now runs the built-in `security-review` skill as a collector whenever the diff touches security-sensitive code, however small — a one-line permission check is the whole attack. It points at the built-in skill rather than carrying `pr-review`'s checklist, for the same reason invocations live in one place.

**Dropped, each for a reason:**
- **The generated-file filter** for scoping review counts. Measured on two real repos over 30-60 days of history: it removed 2-4 files of 600-750, and it would have hidden real source — cody-agent's `resources/views/vendor/mail/*.blade.php` and `widget/build/strip-css-comments.ts`. Cost with no catch.
- **The optimization lens.** The `code-review` skill already covers efficiency, and bundle size is a `/guardrails` tier.
- **The test-quality lens** ("tests that would pass even if the code was wrong"). That is mutation testing, and a gate tier does it mechanically. A project whose gate lacks the tier is a `/guardrails audit` finding, not review prose.
- **The severity tables and parallel-agent layout.** `/polish`'s dedup and Fix/Ask/Park triage replace them.
- **Reviewing someone else's PR by number** — the one use `/polish` does not cover — is served by the built-ins: `/code-review <PR#> --comment` and `/security-review`.

### 2026-09-17 — Checkpoints: the inner loop per slice, the full gate and Prove every K slices

The first two `/deliver` runs (arca PRD-015, 5h40; cody PRD-021, 7h45) felt slower than building by hand. Measured from the transcripts, the framework had not changed; **enforcement** had. By hand, cody PRD-019 built six slices in 62 minutes — no room for a 7-minute gate per slice, and Prove was never run per slice — and the owner's QA reopened the PR the next day. `/deliver` did what the docs said, at 30-35 minutes a slice.

Where the time went, per run: model generation 58-76% of slice time (each subagent starts cold, at xhigh effort), the gate 35% in cody (two hung browser runs cost 35 min alone), Prove 18% of shell time in arca. Orchestrator gaps between slices: under two minutes. Tokens, at API list prices, came out even with hand-driven features per line of code (1.4-1.8¢ arca, 2.7-4.5¢ cody); cache reads were 60% of every run, hand or not.

**Calls, all measured rather than argued:**
- **Slices are sized by build time, 45-90 minutes, at most ~80 files**, replacing "~10 tasks / ~40 files". Per-slice fixed cost is ~12 minutes; a builder's context peaks at 300-550k on a 35-minute slice, so 1.5× is the ceiling — one agent building several slices in a row would cross 1M and compact. Review size stopped being the constraint once `/polish` reviewed the whole branch.
- **Verify per slice runs the inner loop** (`inner:` beside `gate:`, `docs gate --inner`, falling back to the full gate). **The full gate and Prove run at a checkpoint every `checkpoint_every` done slices, default 2** (the owner's call: 2 with bigger slices, 3 if slices had stayed small), and once at the end. Recorded as `CP-n` in `agent-log.md` — no new frontmatter key, per the 2026-08-13 decision — and computed by `docs status`. Dependency-based checkpoints were considered and rejected: both real graphs are chains where nearly every slice has a dependent, so they would have saved nothing.
- **`done` means built, inner loop green, committed** — the owner's words: "we're done coding, let's move on; not set in stone".
- **Parallel slices were deferred**, not refused. Both harnesses document that two drives corrupt each other (shared database, fixed ports, screenshots wiped), and both features were chain-shaped (never more than three runnable slices). The precondition is a harness that isolates instances, built per project; `/prd-to-tasks` now shows lanes and asks, and `/deliver` runs lanes sequentially until then.
- **`Review: decide | fyi | no`** replaces `yes | no`: cody flagged 41 of 63 decisions, which is a list nobody reads.
- **`deliver/bin/measure.py`** ships so a retro's build, polish and token numbers are measured from the transcript. Two retros disagreeing with a feeling is what started this entry.

Retro frictions folded in the same batch: read-only reviewers (a fork moved HEAD under four others), security-review as its own subagent, CodeRabbit scoped or skipped past 150 files, `feat/{NNN}-{slug}` accepted, Cursor's `--mode ask` probed per run (it blocked `git diff` in both runs after allowing it on 09-10), the PRD adversarial pass checking each acceptance criterion has a surface (FR-40), and three `/guardrails audit` items (slow-tier timeout, inner loop declared, diff-scoped tiers reading the working tree). No version bump: every existing doc stays valid, and a project with no `inner:` behaves as before.

### 2026-09-17 — CodeRabbit removed from `/polish`

The owner cancelled the subscription and uninstalled the CLI. `/polish` now collects from local review, the security pass, the UX pass and the Cursor second opinion; `/deliver`'s PR body and the retro template lost their CodeRabbit rows. **This supersedes every CodeRabbit clause above** — the 08-13 ritual, the 09-06 trivial-diff skip, the 09-10 free-tier limits and the 09-17 150-file scoping. Those entries stay as history.

**Unlike Codex on 09-10, nothing was kept for a return.** Codex was paused and expected back, so its plugin stayed installed and the rule became "no probe". CodeRabbit is gone, so there is no binary to probe and no mirror to maintain. Bringing it back is a deliberate re-add to `polish`, `deliver/pr-body.md` and `templates/retro.md`, on the owner's word.

**The cross-vendor argument now rests on one external reviewer.** Agreement between two external reviewers was the stronger signal; what remains is Cursor agreeing with a Claude reviewer, which is still across vendors. Losing CodeRabbit costs little on the evidence so far: it was the lowest signal per minute of the five reviewers on both large unattended runs.

### 2026-09-18 — Tokens: where a run's money goes, and the four levers

On 2026-09-17 four `/deliver` runs (prokeycloud 014, icg-cloud 001, nycase 006, arca-api 012) used a week of the owner's Max quota in one day, as much as the previous five days combined. Measured from every transcript, weighted at API price ratios (a cache read costs 0.1 of an input token, an output token 5):

- **Cache reads were 67% of the cost**, output 16% and cache writes 18%. 62% of it came from calls made with more than 200k of context.
- **Nothing compacted.** No subagent transcript on this machine has ever compacted. Builders grew from 40k to 450k and orchestrators to 690k, because a 1M-context model compacts only near 1M.
- **Every run started inside its `/prd` session**, already carrying 230–590k. About 9% of the day was orchestrators re-reading the PRD interview.
- **By role:** slice builders 37%, orchestrators 25%, fixers 8%, polish reviewers 9%, checkpoints 7%, test-cleanup 4%.
- **By the tool each call issued:** small `grep`/`sed`/`cat` lookups 44%, browser and harness work about 18%. The owner's guess, Playwright, was measured and is second.
- **About half an orchestrator's context is its own retained thinking.** The visible content accounted for only 330k of icg's 691k.

**What changed, and why these:**

- **`autoCompactWindow: 200000` in the owner's user settings**, which a skill cannot set; `/deliver` checks it and says one line when it is unset. Probed before relying on it: with a 100k window, a subagent compacted at about 71k, through both the settings key and `CLAUDE_CODE_AUTO_COMPACT_WINDOW`. A replay of 09-17 put the saving at 28% for 200k, 35% for 150k and 42% for 100k. The replay ignores what an agent re-reads after it compacts, so 200k is the conservative end. It is also the window these agents worked in before 1M became the default.
- **`/deliver` refuses a conversation that already holds work**, and `/prd`'s hand-off says `/clear`. It is the cheapest lever: the run resumes from disk by design.
- **The orchestrator does not investigate.** It is the one agent whose every call carries the whole run.
- **autonomy.md § Tokens: batch lookups, read files once.** Cost is roughly calls × context, so parallel tool calls in one message are the lever inside an agent.
- **An unattended run may declare `inner:` at preflight**, the one gate-config write it is allowed. "Never touch gate config" exists because an agent alone lowers floors. An inner loop has no floor: the full gate still runs at every checkpoint and in CI. Four of five retros hit the contradiction between the tools advising `docs gate set --inner` and autonomy forbidding it.
- **A checkpoint reuses a green full-gate run at the same commit**, and a checkpoint with no Prove rows needs no subagent. Without an inner loop, 42 checkpoint gate runs across four retros repeated the slice's run and caught no application defect.
- **`measure.py` sums sessions and reports `start_context` and `peak_context`**, so the next retros show whether `/clear` and compaction took. The first read was a crash-resumed run whose retro undercounted.

**Deliberately not changed yet.**

- **Effort.** Everything runs Opus 5 at xhigh. Agent definitions accept `effort:` and `model:` in frontmatter, so builders at `high` is the next lever. It is held back so the next retros credit one change at a time, and because the owner rated the output quality.
- **Slice size (45–90 minutes).** Within one agent, cost grows with the square of its call count, so bigger slices cost more per slice. Compaction now caps that. Sizing stays until retros with compaction say otherwise.
- **How many runs go in parallel.** That is the owner's call, not the framework's.

### 2026-09-18 — Five retros folded in: concurrency, proof below the UI, precedence

The rules went into the skills. What follows is only the reasoning a later edit could reverse.

**One test process per checkout.** Parallel agents share far more than files: the test database, probe files a suite writes into the tree, and the orchestrator's git index. All four failures of this kind were the same mistake from different sides. Two `RefreshDatabase` runs wiped each other. A fixer's gate went red on a sibling's half-finished edit. A bare `git commit` swept up a builder's staged deletions. And an agent improvised a scratch database instead of blocking. So the file-overlap rule for splitting work stays, and three more rules join it: no suite runs inside a parallel agent, the orchestrator commits by pathspec, and "cannot share the database" is a blocker.

**A cross-vendor disagreement escalates.** It never cancels. The two findings that most justified the second opinion in nycase 006 were ones a Claude reviewer had explicitly dismissed. For the same reason, `security-review`'s notes are triaged like findings, because its confidence filter parked a real privilege escalation at 3/10.

**The diff goes to Cursor as a file. This supersedes the 2026-09-10 "it runs `git diff` itself" and the 2026-09-17 probe-per-run.** Every run that recorded the probe found shell blocked, so the probe had become a step that always failed and then took the fallback. Reading a file works in every mode. It is also the only form verified end to end on 2026-09-18, when it found four real weaknesses in that day's `autonomy.md` edits.

**The PRD outranks the Never list for a file it names.** The list guards against what nobody approved. A PRD is approved, so following its explicit instruction is not the agent deciding alone. The `AD` is marked `decide`, so the owner still sees it.

**Adding a CI setup step is part of a gate extension.** Removing or skipping a step never is. The Never list exists because agents loosen checks. A toolchain step that lets CI run what the gate newly covers tightens it.

**`reviewed:` has one meaning: `/polish` reviewed the code.** A PRD review once set it, and `/deliver`'s resume table would then have skipped polish. So `/prd` is told never to set it, and preflight refuses it on a `draft` or `ready` PRD, which is a fact a machine can decide.

**`W-SIZE` measures the spec, not the record.** Progress, review fixes and deviations land in the slice as it is built. Counting them flagged three of arca 012's six slices after they were `done`. The warning asks for a split, and splitting a done slice is the one thing the framework forbids.

**Evidence never lives in a session scratchpad.** A crash-resumed run lost CP-1 to CP-3's path and had to cite the old session's folder. Evidence goes in the harness's artifacts directory, or in the feature's `evidence/` folder when there is no harness.

**Deferred, on purpose:**
- `defers:`/`resolves:` slice keys. The prose convention worked in icg 001, and a new frontmatter key needs a second run asking for it.
- A shared-database isolation mode for parallel lanes. It is a per-project harness feature, the 2026-09-17 deferral.
- `docs retro --scan`. Seven retros now exist, but three of them predate the measured keys.

### 2026-09-18 — `/deliver` ends by offering the owner a decision interview

The owner asked for it. The report listed "needs your review" and left the owner to review it one agent-log entry at a time. Now §8 offers an AskUserQuestion interview over every `decide` and `fyi` entry, plus DEC candidates and rubric proposals, with a recommendation first. It applies the answers in one push.

- **Offered, not run.** The run is unattended until it reports. The first question comes after everything is finished, so nothing waits on the answer, and "later" costs nothing. `/deliver NNN` brings the offer back.
- **The recommendation is re-judged, not replayed.** By the end, the run knows what later slices, polish and CI taught it. A recommendation that always says "keep what I chose" is a rubber stamp, which is the failure the interview exists to avoid.
- **Every answer is an owner line in `agent-log.md`**, and the retro gets a kept/changed count. This does two jobs. The lines make the review resumable, since an entry with an owner line is done. And the count is the only direct evidence of whether autonomy.md's decide/log/block line sits in the right place: a high overturn rate on `no` or `fyi` entries means the run decides too much alone.
- **One push, even when it is docs-only.** This relaxes the 2026-09-16 no-docs-only-push rule, on purpose. That rule stopped an unattended run from restarting CI to tidy its own notes. Here the owner is present, and the owner lines are the record that gets merged.

Wiring it exposed a real gap. **Preflight refused every `shipped` PRD**, so a run could not resume at PR, CI or this step, including after a CI blocker. A `shipped` PRD with an `agent-log.md` is now a `/deliver` run to resume. One without it is still refused.

### 2026-09-19 — Extras need evidence, and the gate sits in the `/prd` interview

**The owner's definition of bloat:** building for scenarios nobody has seen. The PRD says "integrate this API" and the build prepares for the provider's datacenter being hit by an alien ship. Defensive code (validation, null checks, `try/catch`, defaults) is welcome and is not an extra. Test bloat belongs to `test-cleanup`.

**Measured before deciding**, over arca-api PRD-012 and PRD-015 (9,383 production lines added). Drift after the PRD (slicing, build, review fixes) was about 400 lines, 4.3%, across six mechanisms. Five of the six entered at slicing, and review fixes added the most lines. The PRDs themselves asked for about 2,000 lines of extras (agent estimates, some overlap). The largest was 012's failure backoff and set-aside, ~850 lines plus a ~710-line release screen. The agent raised it as an OPEN in the interview, when the owner's draft had no such FR. Recon and the adversarial pass grew it, and the owner closed each question by picking a shape. Meanwhile the same PRD's §7 called the failure harmless today.

**Why the gate is at the start**, and not a pass at the end:
- A commit-time check is the most expensive place (the owner rejected it). By then the builder has planned, researched and built the extra, and arguing with the check costs more.
- A cut pass after a free interview asks the owner to reverse decisions they already closed, after the rounds are spent.
- A dedicated bloat adversary collides with `/prd` §4's rule that a reviewer without the interview re-litigates settled calls. Adversaries also add scenarios rather than remove them: on 09-17 the pass on 012 added three OPENs.
- No classifier or hook is needed. The lever is which option the question puts first.

The single cut question at the end stays as a backstop anyway. Extras pass the gate one at a time, and it is their sum that doubles a PRD.

**Left out means a `Not handled:` non-goal, never an OPEN.** An OPEN invites a later stage to settle it, usually by building it. A non-goal binds downstream: autonomy.md already blocks an unattended run from changing one. The "Revisit if" line names the evidence that would reopen it.

**The slicer quotes a source for every task** because slices are where downstream drift starts, and every later stage treats the slice as the spec. "The trap" as written invited what-ifs; one slice's trap note on 012 became a ~145-line stale-tab pin. A trap is now a fact of today's code.

**Deliberately not changed yet.** `/polish` still lets a review fix harden an unrequested mechanism instead of deleting it (~185 of the ~400 downstream lines). A builder can still build code its own `AD` calls unreachable. Both are held back so the next retro credits one change at a time. Replay the same drift count on the next `/deliver` run.

No version bump: new guidance and a template line, and every existing doc stays valid.

### 2026-09-20 — Preflight asks, and why a lane is not worker-safety

**Measured trigger:** icg-cloud PRD-003. 683 build minutes over 13 slices, strictly serial. Walking its own `depends_on` graph with its own per-slice timings, the critical path is 374 minutes — about three hours lost to a scheduler that takes one slice at a time. Four slices (09-12) sit off the path entirely. Meanwhile the token diet held: cache reads per build-minute were 1.61M before it (PRD-001) and 0.73M after (PRD-003), across a feature 4.5× bigger. **The optimizations that shipped on 09-18 were token optimizations. Nothing has ever targeted wall-clock.**

**Why a third preflight bucket.** `judge` lines get nodded past. At the time of writing, every live project on the framework — arca-api, nycase, cody-agent, icg-cloud — has no `inner:` declared, and icg-cloud ran `/deliver` three times against both a standing `judge` line and a `deliver` §1 bullet instructing the agent to declare one. No `chore(docs): declare inner loop` commit exists on any branch. An advisory line inside an unattended run is not a line. So `ask` blocks, exit 2.

**Why it blocks at preflight and nowhere else.** The owner typed `/deliver` and is at the keyboard; they will not be again for eleven hours. That makes it the cheapest interrupt in the system, and the only one where a five-minute fix can still delete a cost the run would otherwise pay on every slice. A commit-time check is the expensive end of the same idea, and was rejected for `/prd` extras on 09-19 for the same reason. `pass` keeps its old meaning (nothing mechanical refuses) so existing callers do not break; asks are a separate array and a separate exit code.

**A lane is not `--parallel`, and this was got wrong once already.** `--parallel` is N workers sharing one process group, which is what icg-cloud's module-config leak breaks. A lane is one agent's private database, ports and scratch paths, so two lanes are two ordinary `composer test` processes that share no memory. **A suite that cannot run `--parallel` still works in lanes.** Worker-safety is worth its own ask — it cuts every individual gate run — but nothing waits on it, and treating it as a prerequisite would have parked the whole design behind a project-side refactor.

**Why a file per lane, not a template in `index.md`.** The K=V half of a lane is the easy half. The half that matters is prose: PRD-003's trap that a new Inertia page needs `npm run build` before tests, writing to a shared `public/build`. No number of databases isolates that, and no schema expresses "ask the orchestrator before you touch it". Every other agent-facing artifact in this framework is frontmatter plus prose for exactly this reason. The frontmatter stays **flat** — the parser is minimal YAML with no nested keys, so `env` is a flow array of `K=V`, not a block.

**`lanes: 1` is the default and means no parallelism**, rather than deriving a count from the graph per run. A number that moves run to run makes two retros incomparable, and the graph can ask for more lanes than the machine or the database server has. The owner sets it once; preflight reports when the graph could use more.

**Agents create their own databases** from the lane's `setup` line, rather than the owner pre-provisioning them. The owner accepted the garbage that leaks from a crashed agent as cheaper than the setup it avoids. Revisit if a run ever fails because the server ran out of databases.

**Deliberately not done yet**, so the next retro credits one change at a time: parallel builders in `deliver` §2, lanes for `polish` §5's fixers (which today ban fixers from running tests purely because they share one database), `docs retro --scan`, and the fourth ask — static analysis dominating red inner-loop runs — which needs retro data nothing reads yet.

No version bump: new optional keys and a new command, and every existing doc stays valid.

### 2026-09-20 — Parallel builders, and the three files they cannot share

**What made this safe to do now.** The lane work landed the isolation half: a builder that owns a database, ports and scratch paths can run its inner loop beside a sibling. What was left was the bookkeeping half, and it is where a naive fan-out silently loses work.

**Three shared files, found by reading the code rather than by reasoning about it.** `docs render --write` rewrites *both* `prd.md`'s slice table and `index.md`, so it is not a per-feature write. `agent-log.md` is one file that every builder appends to. Under N writers the loser of a race loses its decisions with no error anywhere — the worst failure mode in the framework, because that file's entire job is to say what the run decided while nobody was watching. So builders render nothing and append nothing.

**Fragments, not returned text.** The alternative was builders returning their `AD` entries in the return block for the orchestrator to write. Rejected: a fragment on disk survives a dead orchestrator, and the whole framework is built on state surviving the conversation — that is why `status: building` is in frontmatter and not in someone's context. It also costs the orchestrator nothing to read, which matters when orchestrators were measured at a quarter of a run's tokens. Checked before committing: a feature folder only recognises `slice-NN-*.md`, so a `log-slice-NN.md` is invisible to `docs check` and a crashed run leaves its decisions lying in the folder rather than nowhere.

**Merge validates numbering, it never renumbers.** Renumbering is the obvious fix and it is wrong: an entry may cite another by number ("superseded by AD-12"), and a silent renumber turns that into a lie. `docs log merge` refuses the whole merge on a clash, touches nothing, and leaves the fragments on disk. Same reasoning as `docs check` refusing on drift instead of guessing.

**The numbering scheme needs no coordination at all.** Slicing owns `AD-1`–`AD-99`; a builder starts at `{slice}01`; a polish fixer starts at `{k}0001`. Two agents cannot collide without talking, which is the only property that matters when they cannot talk. The first draft gave fixers `AD-{k}001` and it collided with slice-10 — caught by writing the ranges out, not by the tool. Five digits for polish because slices top out at `AD-9999`.

**The "do not pay for the same gate twice" shortcut dies under parallelism**, and §2 now says so. It rests on HEAD still being the last covered slice's commit; with siblings landing in between, that slice's green proves nothing about the branch. It stays for serial runs, which is where it was measured (42 wasted gate runs over four runs).

**`prd-to-tasks` had a stale second meaning of "lane."** It used the word for an independent track in the dependency graph and asked the owner, at slicing time, whether to build them in parallel — gated on the *harness* being able to isolate instances. That gate was always wrong for the build and is now plainly wrong: builders never drive the harness, and the checkpoint that does runs alone. The question is also no longer the slicer's: width is computed from the graph, and whether a run uses it is `lanes:` in `index.md`. What replaced it is the thing slicing actually controls — **every declared dependency costs parallelism**, so a dependency added "to be safe" is now a builder idle for hours.

**Not done yet:** `docs retro --scan`, the static-analysis ask, and `/guardrails audit` writing `gate_parallel`. The first parallel run should be at `lanes: 1` regardless — it exercises fragments, merge and the spawn loop with no concurrency, so a failure has one cause.

No version bump.

### 2026-10-11 — `docs new` creates `agent-log.md`, and `freeLanes` skips blocked lanes

Both came from the owner's review of factory PRD-001 (AD-401, AD-503). The driver had worked around each one; the owner chose to fix the tool and keep the driver's checks only for older vendored copies.

- **`freeLanes` leaves out the lanes of `blocked` slices.** A blocked slice's half-finished work still holds its lane's database and ports, so handing that lane to a new builder shares them.
- **`docs new` creates `agent-log.md` from the template.** Every reader had a "missing log" branch. Its presence now says nothing, so the 2026-09-18 preflight rule keys on *written* entries: a `shipped` PRD whose log has at least one entry is a `/deliver` run to resume, and one with an empty log is still refused.

