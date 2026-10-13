---
name: polish
description: The closing ritual for a finished feature, before the merge — local code review, a security pass on sensitive code, a UX pass over the changed screens and a cross-vendor second opinion, deduped and triaged into fix / ask / park, then test cleanup, then doc closeout. Use when a feature's slices are all done and each Prove task has its evidence, or as the polish worker of a `factory` run, or when the user says "polish", "pre-merge", "wrap this up", "ready to merge", "finish this feature", "review before merging".
argument-hint: [base-branch]
---

# Polish — the pre-merge ritual

The work is built and QA'd. This is everything between "it works" and "merge it".

**The order is deliberate and load-bearing.** Do not reorder it to be efficient.

**Inside a `factory` run, the contract in your pack wins.** It says what you decide alone, what you log, when you block, that you never commit, and what you return. Wherever this skill asks the user, commits, or closes out, the contract's rule replaces it; this file carries no second copy of those rules, because every copy drifted.

## 0. Preflight

- **The bar.** Triage is the judgment half of this ritual: deciding which findings are real, which contradict a settled decision, and which need the owner. It does its best version on a top-tier reasoning model. **Under the bar → one line, then continue.** Silent otherwise. See [_shared/models.md](../_shared/models.md).
- **Is there a diff?** `git status` and `git diff --stat` against the base branch (default: the repo's default branch, or `$ARGUMENTS`). Nothing to review → say so and stop.
- **Framework project?** `factory docs status`. If `docs/` exists, this is a feature closeout and everything below applies. If not, you still do collect → triage → fix → cleanup; skip the framework-specific steps and say which you skipped.
- **Load the arbiters, before reading a single finding.** This is what makes triage sharp instead of noisy:
  - the feature's `prd.md` — especially its **Decisions log** (`OPEN-n`), **Non-goals**, and **§QA plan** (the proof each FR promised — a `done` slice whose Prove task has no evidence in Progress is a finding, not a formality)
  - `docs/decisions.md` — the project's standing engineering decisions (`DEC-n`), if it exists
  - `docs/ux-rubric.md` — what "finished" looks like on screen here, if the diff touches UI
  - `CLAUDE.md` / `AGENTS.md` / `.ai/guidelines/` — the project's own rules. The gate itself comes from `factory docs gate`.
- **Are the slices done?** If any slice is still `pending`/`building`, say so and ask whether to continue — polishing half a feature usually means the review runs twice.

Create a working file in your scratchpad directory (`findings.md`). **Everything goes there as you go.** This ritual is long enough to hit a context compaction, and losing the triage means paying for every review again.

## 1. Collect — scale the reviewers to the diff

**Every reviewer on a one-line bugfix is theatre, and a ritual that always costs an hour gets skipped entirely.** Pick the set from the diff and its blast radius, say which set you picked and why, then run only those:

| Diff | Reviewers |
|---|---|
| A few lines, one file, no user-visible surface | **local review only** |
| One slice, or a bugfix with a regression test | local + the second opinion |
| A feature: several slices, or anything user-visible | local + UX + second opinion |
| Migrations, auth, money, an external contract, a public API | all of them, and say so — blast radius, not diff size, decides this row |

Two rules override the table. **Anything security-sensitive gets the security pass**, however small — a one-line change to a permission check is the whole attack. **Anything user-visible gets the UX pass**, however small — a one-line CSS change is exactly where a state breaks.

**Every reviewer is read-only on the checkout: no `git switch`, no `checkout`, no `stash`, ever.** Reviewers run side by side, and one that moves HEAD to compare against the base branch pulls the tree out from under the others mid-read — it happened. A reviewer that needs the base tree gets a worktree (`git worktree add`), never the shared checkout. Say this in every reviewer's prompt.

**Local review first**, always. Invoke the `code-review` skill. It has full repository context and its findings are the higher-signal set; running it first also stops the second opinion's noise from anchoring the local pass.

**Then `security-review`, when the diff touches security-sensitive code** — authentication or authorization, a permission or tenancy check, parsing or validating outside input, raw queries, file paths or shell commands built from input, deserialization, crypto, secrets or config, CORS, headers or cookies, or logging that could carry user data. The general review does not look through that lens. Skip it only when none of that changed, and say so. Its findings join the same triage. **Run it as its own subagent**: its contract ends the turn on a report, which is right for a standalone review and wrong for a collector in the middle of a ritual.

**Then `/ux`, when the diff touches anything a user sees** — pages, components, templates, styles, or the server code deciding what a page renders. It drives the app through the verify skill, captures every state (empty, loading, error, focus, narrow, long content) and checks them against `docs/ux-rubric.md`. It is a **collector like the others**: its findings join the same triage, not a separate ritual.

Skip it only when nothing user-visible changed, and say so. No harness (`.claude/skills/verify-*` missing) → say that instead, and that the UI went unreviewed; do not substitute reading the source.

**Then a second opinion that did not write the code.** Use the `second opinion` role in [_shared/models.md](../_shared/models.md) — Cursor CLI, unconditionally. Do not probe for any other vendor's CLI or plugin and do not branch on one being installed; presence says nothing about whether its subscription is live. Cross-vendor is the entire point: models from one vendor share blind spots, so a second Claude agreeing with the first is weak evidence.

**Run `factory review second --diff <base-branch>`.** It checks `--list-models` before it spends anything, splits the diff into passes of about 600 changed lines, runs each pass read-only under a timeout, and prints one section per pass with the log path last. Scope it with `-- <paths>` when the diff mixes risky and trivial files. When the local review or the security pass found high-severity defects, write their one-line summaries to a file and add `--others <file>`: the reviewer is then asked what fixing those would still miss, which is how Cursor found three escalation paths two Claude reviewers had missed. The agent and the model are `[review]` in `~/.config/factory/config.toml`, never this file.

**Read its exit code.** `0` is findings: claims to verify in step 2. `2` is unreviewed (the auth is gone, the model is missing, or no pass returned text), so the fallback below starts now, said out loud. `1` is a usage error: fix the command and run it again. A pass marked `unreviewed`, or `timed out … partial`, is never *no findings*: name the files it did not finish.

Cursor unavailable, or the run fails → a fresh agent on the judgment model with **no inherited context**, never a `fork`. Either way its findings are claims to verify in step 2, and you say which reviewer produced them.

## 2. Verify and dedup — one pass over every set

**The second opinion does not have the context you do.** It has not read the PRD, does not know what was deliberately decided, and cannot see the reasoning behind a design. Treat its findings as claims to check, not instructions.

**Where the second opinion and a Claude reviewer agree independently, weight it up.** That is the reason the second opinion is cross-vendor: different vendors have different blind spots, so agreement across vendors is real signal, while two Claude reviewers agreeing is mostly correlation.

**The converse matters more: disagreement escalates, it never cancels.** When one reviewer raises a finding another explicitly dismissed, verify it at `file:line` as if nobody had dismissed it. In one run the two most valuable findings were exactly that: Cursor raising what the security pass had called "already tenant-visible".

**A collector's notes are findings too.** `security-review` holds back anything under its confidence bar and lists it as notes. One run parked a real privilege escalation there at 3/10. Triage every note like a finding.

For every external finding:

- **Verify it against the actual code**, at `file:line`. A finding that misreads the code gets a **one-line rebuttal recorded in the report, not silently dropped** — the user must be able to disagree with your pushback.
- **Check it against the PRD's decisions log and `docs/decisions.md`.** A finding that contradicts a settled `OPEN-n` or a `DEC-n` is not a bug — it is re-litigating a decision. Flag it as such, with the citation. This is the single most common false-positive class from a context-less reviewer.
- **Dedup against the local findings.** The two reviewers overlap. Merge duplicates into one entry keeping the better explanation, so nothing gets considered — or fixed — twice.

Triage the merged set **once**, not each batch separately.

## 3. Triage into three buckets

**The test: does the finding determine its fix?**

| Bucket | When | Example |
|---|---|---|
| **Fix** | The finding names the problem *and* the repair is forced | off-by-one, missing guard, wrong status code, unhandled rejection, missing test |
| **Ask** | The finding is real but the remedy is a genuine choice, or it contradicts a settled decision | "this couples X to Y", "should this be cached or fixed at the query level", "this behaviour is wrong" with no forced answer |
| **Park** | Real, but not this PR's job — it would touch files outside the feature or is a slice's worth of work | a refactor the finding merely brushes against |

**Do not ask permission to fix the Fix bucket.** That gate always gets the same answer; it is friction, not safety. State what you are fixing and fix it.

## 4. Ask — one batched round

Use **AskUserQuestion**, one round covering every Ask-bucket item. Per item: the finding, 2-4 concrete options, and what each costs.

**Consult `docs/decisions.md` first:**

- **An exact match to an existing `DEC-n`** → do not ask at all. Apply the prior decision, cite it, and list it under what you fixed. The user already answered this.
- **A near match** → still ask, but put the prior decision first as the recommended option, labelled *"(Recommended — consistent with DEC-7)"*.

Follow-up questions only if an answer opens a genuinely new fork.

## 5. Fix

Apply the Fix bucket plus the user's answers. **Write regression tests freely** — do not economise on them here. That is what step 7 is for.

A long Fix list splits across subagents **only when their file sets do not overlap** — server and UI, say. Two agents editing one file in a shared checkout produce a silent merge; when the sets touch, run them one after another. A split also shares everything that is not a file, so give each fixer these rules in its prompt:

- **Tests in a fixer depend on lanes.** `factory docs lanes` says what the project has. **With lanes declared, hand each fixer one** — `docs/reference/lanes/lane-{L}.md` is its database, its ports, its scratch paths — and it runs the tests it touches freely; a lane is a separate process, so a suite that cannot run `--parallel` is still fine. **With `lanes: 1`, no test suite runs in a fixer**: they would share one database and whatever probe files the suite writes into the tree. Fixers then run the single tests they touch, one process at a time. Either way you run the full gate once, after all of them return.
- **Run static analysis on the whole project, not per file.** It is read-only, so it is safe in parallel. Per-file runs miss errors that only show in class context.
- **A red result may be a sibling's half-finished edit.** Wait and re-run; do not fix code outside your own set.
- **Report what you could not reach, not a workaround.** A route the other side owns, or a key the other side broke. When all fixers are back, reconcile what the split stranded before you run the gate.
- **Hand each fixer its own `AD` numbers**, or have it return its decisions for you to number. Two fixers counting from the same log will collide. Inside a `factory` run, give fixer *k* the file `{feature dir}/log-fix-{k}.md` and the range `AD-{k}0001` (five digits: a slice's range tops out at `AD-9999`, so polish can never land on one), and fold them in with `factory docs log merge <NNN>` once every fixer is back — it refuses on a clash instead of merging one over another. **Tell each fixer its fragment starts with `## Decisions`**: merge refuses a title line or a prose preamble, and stripping them by hand happened twice in one run.

Run the gate:

```
factory docs gate
```

prints it. Without the tool, or when it exits 1, use what `CLAUDE.md` / `AGENTS.md` / `.ai/guidelines/` names — and if nothing names one, say so rather than inventing one. That is a `/gate` job, not a polish job.

## 6. Park

Each Park item becomes either an idea, `factory docs idea add --title "…" --origin "PRD-NNN polish" --body-file -`, its body carrying enough recon that a future PRD does not have to rediscover it, or a new slice if it is genuinely this feature's remaining work. Without this step, parked findings are simply lost.

## 7. Test cleanup — last, and this is why

Invoke the `prune` skill in **`pr` mode** (only tests added or changed on this branch, not the whole suite).

**It runs last on purpose.** Step 5 should be free to add regression tests without a pruning pass breathing down its neck; rationalising them is a separate job that happens once, after all the fixes exist. **Do not move this earlier** to save a pass — the ordering is the point.

## 8. Distil — three destinations, then the scratchpad dies

- **Fixed** → the slice's `**Code-review fixes ({date}).**` section, one bullet each: what was wrong, why it mattered, what it is now. Feature-wide fixes go in the last slice touched, or the PRD's Status prose if they belong to no single slice.
- **Decided** → candidate `DEC-n` entries in `docs/decisions.md` (see below).
- **Parked** → already handled in step 6.
- **Rebutted** → keep the one-line rebuttals in your report to the user. They do not need a permanent home, but the user must see them.

Then set `reviewed: {date}` in the PRD's frontmatter.

### The correction loop — one question, every time

**Whenever the owner fixes something none of the reviewers caught, ask what would have caught it.** That answer has a home:

- a design rule the review missed → a line in `docs/ux-rubric.md`
- a rule a machine could enforce → a lint rule, an arch test, or a gate tier via `/gate`, and then it does *not* go in the rubric
- a recurring engineering fork → a `DEC-n`, under the three tests below

Without this, the same correction arrives by hand on every feature. It is the cheapest step in the ritual and the one most often skipped.

### Which decisions earn a `DEC` entry — recommend, do not assume

**Most do not.** A bloated decisions log defeats its own purpose: it exists to be searched three PRs later, and noise makes it unsearchable.

Propose an entry only when all three hold:

1. **It could recur.** The question is about a *class* of problem in this codebase, not one file that is about to change anyway.
2. **It was not obvious.** If the answer follows from an existing convention, the convention already covers it.
3. **You can write a real "Applies when" line.** If the trigger is "in `AgentController::edit()`", it is not a decision, it is a fix. **This is the sharpest filter — use it.**

**Recommend, with your reasoning, and let the user choose.** Default is not to log. Say something like: *"Two of these five are worth logging as DECs — the caching one and the status-enum one, because both will come up again the moment we touch another aggregate. The other three are local."*

Entry shape:

```markdown
### DEC-7 — {the question, phrased as a question} · {date} · {PRD-013 slice-03}

**Chose:** {what}
**Over:** {the alternative, and why it lost}
**Because:** {the reason that will still make sense in six months}
**Applies when:** {the trigger a future agent will recognise}
```

Numbers are monotonic and never reused. A reversed decision is **superseded, not deleted**: leave the old entry and add `**Superseded by DEC-12.**`

## 9. Close out

**Commit as you go — do not leave any of this staged-but-uncommitted for the user to finish.** Fixes commit separately from the `/prune` pass, so the diff for each stays readable.

The closeout is one final commit:

- the prose **Status** line in `prd.md` — what actually landed
- the index **Notes** cell — decisions taken, what remains, the PR number
- any new `DEC-n` entries the user approved
- then `factory docs ship <NNN>` (something outstanding survives the merge) or `factory docs archive <NNN>` (nothing left at all)
- `factory docs check` clean **before** committing — never commit docs that fail it

Pushing still asks. Everything up to the push is yours to do.

## Report

Inside a `factory` run these are the sections of the PR body, in the shape the pack gives, and the `FINDINGS` line the contract asks for; otherwise a chat report.

- what each reviewer found, **named** — local, security, UX, the second opinion and which model or vendor produced it — and how many findings were duplicates across them. Say plainly if the security pass, the UX pass or the second opinion was skipped, and why
- **what you fixed**, grouped, one line each
- **what you rebutted and why** — the user may disagree
- what you parked, and where it went
- which decisions you recommend logging as `DEC`s, and which you deliberately did not
- what `/prune` removed or consolidated
- gate output, and the closeout state
