---
name: harvest
description: Walk the owner through what a batch of shipped features taught — each retro lesson and each idea they spawned — and route every one to where it changes the next feature: this project, its framework knobs, factory itself, or nowhere. Then compact docs/decisions.md into named entries, archive the batch with `factory docs harvest --done`, and commit. Attended only. Use when shipped features pile up, when `factory docs status` lists features as shipped and the owner wants them archived, or when the user says "harvest", "archive the shipped features", "what did these features teach", "route the retro lessons", "convert decisions.md".
argument-hint: [NNN…]
---

# Harvest — every lesson goes somewhere, then the batch is history

A shipped feature's folder holds what the work taught: its retro, the friction it met, the ideas its review parked. Left there, it teaches nothing: `docs/features` and `docs/archive` are history, and the code, `docs/reference` and `docs/decisions.md` win over them. This skill reads the batch's lessons, routes each one with the owner to a place the next feature will read, then archives the batch to its records.

**Attended only.** Routing a lesson is judgment, so the owner answers every question here. No `factory` run calls this skill and no worker plays it.

## 1. Read the lessons

```
factory docs harvest [NNN…]
```

With no numbers, the batch is every `shipped` feature; the owner can name a smaller one. A named feature that is not `shipped` is refused with its status: ship it first, or leave it out. The verb only reads. It prints, per feature, the retro's sections without its Numbers block, then each idea whose origin names the feature, by id and first sentence.

**Report the list before you ask anything.** One line per lesson: the feature, the lesson in a sentence, and where you would route it.

## 2. Route each lesson

**AskUserQuestion**, recommendation first, one round of up to four lessons at a time. Every lesson goes to exactly one of four places:

- **The project**: a diff in this repo. The code, the harness (`.claude/skills/verify-*`), the gate, `CLAUDE.md`, `docs/reference`, `docs/ux-rubric.md`, or an entry in `docs/decisions.md` in the form of §3. Make the edit now; it lands in this skill's commit.
- **The framework in this project**: a knob in `docs/project.toml`, such as the gate, the lanes or a model role.
- **Factory itself**: the skills or the binary were wrong for everyone. See below.
- **Nowhere**: the lesson was local, or it is already handled. Say so aloud, one line, so the owner can disagree.

An idea the batch spawned is a lesson too. Routed to the project, it becomes the edit or stays an idea for a future PRD. Already built, or no longer true, it is deleted with the owner's yes.

**Factory itself.** When this repo is the factory checkout (its `.claude/skills/` link into `skills/`), file it here: `factory docs idea add --title "…" --origin "harvest {date}" --body-file -`. Elsewhere, the lesson leaves this repo, so:

1. Write the draft as a markdown file in your scratchpad, outside the repository: a `# ` title line, then the body. Write it **in general terms**: what the skill or verb did, what it should have done, a minimal shape that shows it. No customer, no repo name, no path from this machine, no code that is not the framework's own.
2. Show the draft to the owner.
3. `factory upstream check <draft>`. Each hit is one line: rewrite the draft and check again until it is clean. The check is the floor, not the review: the owner reading the draft is the real one.
4. The check lists open factory issues that match the title. On a real match, offer to comment on that issue by hand instead of filing a repeat.
5. `factory upstream post <draft>` only on the owner's yes.

## 3. Compact `docs/decisions.md`

One diff the owner reviews on its own, `git diff -- docs/decisions.md`, before §4. Skip it when the file does not exist.

- **An entry** is `## {Short name}`, then **Chose**, **Over**, **Because** and **Applies when**. No number, no date, no origin in the heading or the body. Its link is the heading's own anchor: the slug GitHub makes from it (`## Who talks to GitHub?` → `decisions.md#who-talks-to-github`).
- **Old links keep resolving.** An entry from the numbered form keeps its old anchor as a line above its heading: the first numbered entry gets `<a id="dec-1"></a>`, and so on. A renamed or merged entry keeps every old slug the same way, one `<a id="…"></a>` line each.
- **A reversed or merged loser folds into the winner's Over**, with the reason it lost. The loser's heading goes; its anchors move above the winner's.
- **An entry whose Applies when no longer exists in the code stays whole.** An Over is never lost.
- **A domain fact is no decision.** Move it to the doc that owns the topic and leave a one-line pointer here, under its anchors.
- **The file's rules block** says the same: no numbers, anchors kept, losers folded.

Rewrite nothing outside `decisions.md`. Logs, PR bodies, retros, the archive and other records keep the ids they cite, and the kept anchors make those links resolve.

## 4. On this repo's first harvest

The first harvest is also the migration. Check each; do the ones still missing:

- **The convention line** in `CLAUDE.md` and in `AGENTS.md`, inside the factory convention block: "`docs/features` and `docs/archive` are history: the code, `docs/reference` and `docs/decisions.md` win." A new repo gets it from `factory init`; an existing one gets it here.
- **The archived definition** in `docs/index.md`: an archived folder is moved under `docs/archive/`, holds its records only (`prd.md`, `state.toml`, `log.jsonl`, `retro.md`) and carries the history banner. `/harvest` routes the lessons first; no verb checks that, so the definition does not promise it.
- **Home paths outside the batch.** `factory docs check` reports each tracked doc holding `/home/<name>/` or `/Users/<name>/` as `W-HOME-PATH`. `--done` rewrites the ones it keeps; rewrite the rest by hand to `~/`, such as an idea's From line or a research note §5 keeps. A file `--done` drops needs none. Neither the check nor `--done` sees a home with no `/` after the name (`find /home/<name> …`): grep the kept files for one.

## 5. Archive

**Keep what is worth keeping first.** `--done` keeps only the four records and drops the rest of each folder to git history: slices, qa-guides, research notes, evidence. A file a future feature should read moves to `docs/reference` now, with the owner's yes.

**Then commit what you have.** The verb refuses a folder with uncommitted changes, and a keeper moved out or a home path rewritten inside a batch folder is one. On the default branch, create `harvest/<yyyy-mm-dd>` first; on any other branch, stay. Commit every edit §2 to §5 made: `docs: route harvest NNN…`.

```
factory docs harvest NNN… --done
```

Name the batch you routed, never the bare form: a feature shipped since step 1 would be archived unread. The verb refuses a folder with uncommitted changes and a folder already in `docs/archive/`. It stages what it moved and dropped, and commits nothing.

**Then check the links.** The commit hook runs `factory docs check` on any staged `docs/` path, and a live doc linking into a moved folder fails it:

```
factory docs check
```

Fix each `E-LINK-BROKEN`: point the link into `docs/archive/`, or at the file's new home in `docs/reference`, or drop it when the target went to git history. Run it again until it reports no errors.

## 6. Commit

On the branch §5 committed to, one commit with what `--done` staged and the link fixes: `docs: harvest NNN…`. **Never push.** Never `--no-verify`: a hook that refuses is §5's check, not in the way.

## Report

- each lesson and where it went, grouped by the four places, with the dropped ones named
- what went upstream: the issue or idea, or the match the owner commented on
- the `decisions.md` diff in a line: entries renamed, merged, folded, moved
- the archived features, the files kept elsewhere, and the branch and commit
