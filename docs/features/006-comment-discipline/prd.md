---
number: 006
slug: comment-discipline
title: "Comment discipline"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-10-14
author: Emiliano Tisato
summary: |
  Workers write a comment only when it says what the code cannot: an invariant, a foreign quirk, a public contract. They never write a ticket id, a history note or change narration. Specs stop asking for comments. Polish checks every added comment, and the retro tracks comment share. A cleanup skill weeds the existing code, because agents copy what is nearby. **Draft 2026-10-14, no interview yet:** recon and evidence are in, and OPEN-1 to OPEN-6 hold it. Research is in `research.md`.
---

# PRD-006 — Comment discipline

Drafted from one session's research (2026-10-13/14): a 9-agent web and repo sweep with a skeptic per sweep (54 claims, 52 held), plus a comment audit of two field repos. The full write-up is in [research.md](research.md). **The interview has not run.** Every decision below is a proposal until the owner settles its OPEN.

## 1. Why now

**Both field repos were built 100% with this framework** (V1 skills, V2, then the v3 CLI; owner, 2026-10-13). So every number here is ours, including each audit's "before" baseline, which is just an earlier framework version.

**Comment share grew with the framework.** These are comment lines as a share of added non-blank lines, measured 2026-10-13:

| Repo | Earliest era | Middle | Latest (v3 factory) |
|---|---|---|---|
| cody-agent (Laravel/Vue), src | 8.6% (06-13 to 07-18) | 20.4% (005-014), 20.6% (015-020) | 26.5% (021-025) |
| arca-api (Go), src | 4% (05-23 to 07-03) | 19% (07-27 to 09-15) | 21%; feature 012 alone 49% |
| factory (Rust), src+tests | — | — | 4,910 comment lines in 50,062 code lines; **692 (14%) cite an id**, in 100 files |

Docblocks are 68% of cody's framework comments. In arca, the share of new **private** functions with a doc comment went from 5% to 39%, then to a 64% median.

**Bugs the owner actually hit:**

- **A wrong security claim.** `cody-agent/app/.../DynamicSetResolver.php:140` says the widget re-checks the link scheme. It does not. Flagged 09-06 in cody's `docs/ideas.md:96`, still wrong.
- **A comment misled a worker.** A script comment promised 53 tools when there were 55 (cody `docs/features/027-*/agent-log.md:17`).
- **Stale "until slice N" notes for shipped slices.** cody `AssetIngestService.php:31` "PDF ingest path lands in slice 02", while `ingestUploadPdf` exists at `:56`. cody `ContentFeedSyncCommand.php:10` and `AgentRunner.php:293`. arca `internal/caea/cap_policy.go:30` "Slice 04 makes these editable per register… Until then", which shipped in e947135. cody `AgentFaqController.php:20` names a class that no longer exists.
- **A stale comment became a template.** arca's slice `012/slice-04-chain-backoff.md:271` told the builder to document constants "like `DefaultCaps` (cap_policy.go:26-42)". That is the stale comment above, and the builder copied it into `internal/caea/informe_backoff.go:16`.
- **Ids that resolve wrong.** Ids restart per PRD, so a bare id in code is ambiguous:
  - arca's "OPEN-5" appears 11 times and means different questions in PRD-012 and PRD-013.
  - cody's AD-12 exists in both 021 and 025, and 79 of its 124 AD references name no feature.
  - In factory, `src/closeout.rs:2176-2179` cites AD-1 and AD-30001, and neither is the decision meant.
  - In factory, `src/checkpoint.rs:614` "(FR-9)" means PRD-001's quota FR, while PRD-005's FR-9 is `/lanes`.
- **A comment that reads backwards.** `src/scheduler.rs:399-401` says a due checkpoint with no free lane waits. The flag below it is `board.checkpoint_due && !free.is_empty()`, true when a lane **is** free.

**Agents copy what is nearby.** This is lauren's ([@poteto](https://x.com/poteto/status/2110149626133438732), 2026-10-13) claim, and both audits confirm it:

- **cody:**
  - Ticket tags grew from 0.7% of comment lines to 5.5%, 7.2%, then 12.4%. By feature, they went from 4-6% of blocks (005, 007, 011) to 39% (021) and 51% (025).
  - `AD-n` tags started in one file in 021 (`SecurityController.php:48`, 954cb1c) and reached 46 blocks by 025.
  - A browser-test header, "FR-n on the surface X touches", spread from 1 file (019) to 5, then 7, then 6 files.
  - Stub docblocks from 005 were copied by 024.
- **arca:**
  - `// FR-NN:` test headers grew from 1 test to 45. They open 36% of 021's test comments.
  - Markdown bold inside Go comments started in one CAEA file. Feature 012, working in that package, added 67. Features in other packages added almost none.
  - Two runs of the same PRD (024) tagged 14% and 2% of their comments. So tagging is run noise, not a need of the PRD.
- **factory:** density did not rise (8.9% of lines on both `main` and this branch), but the ids carried over. 307 of 435 plan references name no PRD. The wrong AD refs at `src/closeout.rs:2176` were carried from PRD-004 into PRD-005.
- **The evidence beyond our repos.** A TODO that described a problem made Copilot reproduce the debt in 29 of 82 cases ([O'Brien et al., ICSE 2024](https://doi.org/10.1145/3597503.3639176)). The density-copying correlation itself is weak (r 0.31-0.47, small samples), though all three repos agree.

**Where lauren goes too far.** The tweet is one practitioner's opinion, not evidence. "Ban most if not all" deletes what both audits say a reader would miss: concurrency invariants and "deliberately" notes. And what spreads here is plan ids and test headers, not workaround notes. One counterpoint: TODO and lint-suppression markers had almost no effect on Claude Code on Sonnet 4.6 ([SonarSource](https://arxiv.org/abs/2605.20049)).

**Why the default rule does not hold.** Workers already get Claude Code's own rule, from the 2.1.296 system prompt: default to no comments, write only a non-obvious WHY, and never cite the task or issue, because those "rot". `src/worker.rs:307` appends the pack to that prompt and does not replace it. On this machine, workers also get the owner's output style, "Fewer comments beat more"; all 16 PRD-005 worker streams show it. Yet 14% of factory's comment lines cite an id. A rule in the default prompt loses to three things:

1. **The specs ask for comments.**
   - In cody, 25 of 148 slice files mention comments: "State the invariant in a comment" (013/slice-01:56), "a code comment pointing at slice 03" (024/slice-01:45).
   - In arca, 012's PRD and slices mention comments 36 times, and 012 is the densest feature.
   - Factory's skills neither ask nor forbid: `grep -i comment skills/ assets/` finds only `assets/lookup.md:4`. Its own slices do ask: "Write it in the doc comment" (`docs/features/004-skills/slice-01-spine.md:79`).
2. **The packs are thick with ids, and workers echo them into code.** `/slice` makes every task trace to the PRD (`skills/slice/SKILL.md:86`). Builders number their decisions AD-n (`assets/contract-slice.md:17`).
3. **Workers match the comment density they see** (the spread above).

**What the evidence says.** Full sources are in [research.md](research.md).

- **Correct comments barely help frontier models.** Correct docstrings did not raise GPT-4's pass rate ([Macke & Doyle 2024](https://arxiv.org/abs/2404.03114)). The one agentic ablation found, a blog, showed stripping comments slightly *helped* GPT-5-mini on SWE-bench Verified ([comment ablation](https://antimemetics.institute/comment-ablation)).
- **Wrong comments hurt, and models trust them.** Misleading comments cut accuracy about 24% across 17 models, and telling the model to ignore comments barely helped ([CodeCrash, NeurIPS 2025](https://arxiv.org/abs/2504.14119)). A wrong "returns X" comment cost a reasoning model up to 28%. Misleading comments hurt fault localisation more than misleading names ([Haroon et al., ICST 2026](https://arxiv.org/abs/2504.04372)).
- **Agent-written comments are often false.** GPT-4's Java comments held a false statement about 1 time in 5 ([Kang et al.](https://arxiv.org/abs/2406.14836)). Changes that left a comment inconsistent preceded bugs 1.5x more often ([Radmanesh et al.](https://arxiv.org/abs/2409.10781)).
- **A comment is the one note an agent actually sees.** In the owner's 1,731 sessions, agents read code in about 60-line windows. 83% of mid-file reads included a comment line, and `git blame` ran in 0.6% of sessions. So the few comments worth having belong inline, and narration belongs in the commit, where only humans look.
- **Comments cost tokens on every read.** They are 21.8% of the characters in factory's `src/`.
- **Vendors agree:** default to none. See Claude Code's [why-only guidance](https://github.com/Piebald-AI/claude-code-system-prompts/blob/main/system-prompts/system-prompt-comment-why-only-guidance.md) and Codex's ["should be rare"](https://github.com/openai/codex/blob/main/codex-rs/core/gpt_5_codex_prompt.md).

**Where this lands against `/no-comments`.** lauren's tweet points at pstack's `/no-comments` skill, which is open source in `cursor/plugins`:

- **The skill:** [pstack/skills/no-comments/SKILL.md](https://github.com/cursor/plugins/blob/main/pstack/skills/no-comments/SKILL.md). It spawns the agent below, fixes accepted findings, and offers to encode each constraint as a type, test or lint.
- **The agent it spawns:** [pstack/agents/comment-sicko.md](https://github.com/cursor/plugins/blob/main/pstack/agents/comment-sicko.md). It holds the keep-list.

It deletes everything except license headers, foreign quirks, public API contracts, issue links and justified lint suppressions. "When I am not sure a keep clause applies, the comment dies." Our own surprises get marked `MUST KILL` for a rename or refactor instead. FR-5's skill borrows this shape.

Under that list, 8% of cody's sample survives and 18% of arca's. The keep-list is close to the right line. Factory changes three things:

1. **Keep invariants in our own code that no rename or type expresses**, like races, coupled timeouts and cross-module couplings (OPEN-1). The strict list deletes exactly the class both audits say a reader would miss. Examples: arca `internal/http/manage_apikey.go:159`, `internal/queue/jobs.go:381`; cody `PromptCompiler.php:237`.
2. **Allow links to external trackers only** (an upstream issue, an RFC). Internal ids rot.
3. **An unattended run parks a `MUST KILL` refactor as an idea** instead of redesigning code the PRD did not ask it to touch.

**Leaves alone:** prose in `docs/`, READMEs, commit messages and PR bodies; per-language linters.

## 2. Blast radius

- **Every worker in every repo on factory.**
  - `assets/autonomy.md` goes into all six packs (`src/pack.rs:236,293,377,484,569,620`).
  - Workers will write fewer comments, and polish will delete some they wrote.
  - The section costs tokens on every call; `assets/autonomy.md:62` says why that matters.
- **The skills `/slice`, `/prd` and `/polish`.** Their text changes. A skill edit means a version bump, a tag and `factory skills update` in each repo.
- **A new CLI verb, `factory comments`.** It is a new surface and breaks no existing contract.
- **The retro numbers table** (`src/retro.rs:162`) gains a column. Old retros keep their table.
- **Existing code is not rewritten by this feature.** The cleanup (FR-5) runs only where the owner points it.
- **No external contracts.**

## 3. FR-1 — A comment rule every worker reads

**Today.** No factory asset or skill says anything about comments (`grep -i comment skills/ assets/`). Workers get only the default prompt and, on this machine, the output style. The `assets/autonomy.md` sections are: Who decides, The test, Mapping, What an unattended run may write, Never to get green, Tokens, Caps, Blocker protocol, The owner answers mid-run, Resuming.

**Proposed (2026-10-14).** Add a `## Comments` section to `assets/autonomy.md`, so every role reads it: build, fix, merge, checkpoint, polish and CI fix all write code. Draft text:

```
## Comments
- Default to none. The next worker reads, trusts and copies every comment you write.
- Write one short line only for what the code cannot show: an ordering, race or
  cross-module invariant in our code that no name or test expresses; a quirk of a
  dependency, platform or protocol we cannot change; or why a choice is deliberate.
- A public contract (a library's pub API, an HTTP endpoint, a CLI output format) gets a doc
  comment that states it and its edge cases. Never repeat the name or the signature.
- Never cite a PRD, FR, AD, CP, OPEN, slice, task or section. Write the condition itself.
  A link to an external tracker (an upstream issue, an RFC) is fine.
- Never narrate the change, its history ("until slice 4", "for now", "previously") or why
  it is correct. That goes in Progress, the log or the PR body.
- No TODOs. Park the work with `docs idea add`.
- Match the file's idiom, not its comment density, headers or tags. When you change code,
  fix or delete its comment.
```

**Where it lives.** The research synthesis put it in `assets/contract-slice.md`, next to the trap line (`:18`). This draft puts it in `autonomy.md` instead, for the reason under "Alternative that lost".

**Accepted cost.** About 200 tokens on every call of every worker. We take it because one wrong comment costs more: about 24% accuracy for the next reader.

**Alternative that lost.** Putting it in `contract-slice.md` only. Fix, merge and polish workers write code too, and the comment at `src/closeout.rs:2176` came from a polish run.

**Acceptance.** Every role's pack holds the section; a unit test over `pack.rs` checks it. A live run's added comment share and id hits come in below PRD-005's: 11.9% of added lines and 14% of comment lines citing ids (FR-4 measures it).

## 4. FR-2 — Specs stop asking for comments and stop putting ids into code

**Today.** The field repos' slice briefs ask for comments ("state the invariant in the comment", arca `012/slice-03-one-scheduler.md:222`). The densest feature has the most such asks. `/slice`'s "trap" lines (`skills/slice/SKILL.md:83`) are written for the builder, and builders copy them into code.

**Proposed (2026-10-14).**

- **`/slice` and `/prd`:** never ask for a code comment. An invariant the slice must keep becomes a test that pins it, or a type or a name. The trap is for the builder, not the code.
- **The skills and `contract-slice.md` state where ids live:** ids belong to the docs only. Code, test names and comments never carry PRD, FR, AD, CP, OPEN or slice ids. FR-to-test tracing already lives in the § QA plan proof rows and the evidence.

**Accepted cost.** `grep FR-12 tests/` no longer finds the test. The QA plan row names it instead, and a bare id is ambiguous anyway (OPEN-5 in arca, AD-12 in cody).

**Acceptance.** `grep -rniE "in a comment|code comment" skills/ assets/` finds only the ban. A small PRD sliced after the change has no slice file asking for a comment (read back).

## 5. FR-3 — Polish checks every added comment

**Today.** `/polish` has no comment step (`skills/polish/SKILL.md`), and nothing measures comments.

**Proposed (2026-10-14).**

- **A verb, `factory comments check [--base <ref>]`.** It reads the added lines of `git diff -U0 <base>...HEAD` and finds comment lines through an extension table: rs, go, php, js/ts/vue, py, sh, toml/yaml. It reports three things:
  - **Ids:** `(PRD|FR|AD|CP|OPEN|OQ|DEC|BLOCK|ASK)-\d+`, `slice[- ]?\d+`, `§`.
  - **History words:** "until slice", "for now", "previously", "no longer", "added in".
  - **New comment blocks of 3 or more lines.**
  - **The comment share** of the added lines.

  It exits 1 on any id hit.
- **Polish runs it beside its reviewers in § 1 Collect** (`skills/polish/SKILL.md:29`). Its hits pass § 2 Verify like any reviewer's and land in § 3's Fix bucket. Polish fixes them itself, with no question to the owner.
- **Polish's code review gets one question per surviving added comment,** starting with any security claim: does it still match the code beside it, and would a reader miss it? A grep cannot catch the `scheduler.rs:399` class, or cody's false security claim; a reader can.

**Accepted cost.** A new verb, and a comment-syntax table to maintain per language.

**Alternative that lost.** A grep written into the skill text. It cannot be unit-tested, and each repo would drift its own copy.

**Acceptance.** On a fixture branch with seeded comments (an id, an "until slice 4", a real why), `factory comments check` lists the first two, leaves the why alone, prints the share and exits 1.

## 6. FR-4 — The retro tracks comment share

**Today.** The retro's numbers table (`src/retro.rs:162`, between `<!-- factory:numbers -->` markers) has tokens, slices and tokens per slice, but no comment figures.

**Proposed (2026-10-14).** Add two columns, filled from `factory comments check` at ship time: the added comment share and the id hits. They are compared with the three earlier runs, as the table already does. This is how the owner sees whether FR-1 and FR-2 work, run over run.

**Accepted cost.** One more measurement at ship and a wider table.

**Acceptance.** A stub run to ship writes a `retro.md` whose numbers table holds both columns.

## 7. FR-5 — A cleanup skill for existing code

**Today.** The weeds are already planted: 692 id-citing comment lines in factory, 552 bare ids in arca's Go code, and the cody tags above. Because agents copy what is nearby, a rule that only applies going forward keeps losing to them.

**Proposed (2026-10-14).** Add a `/comments` skill, a weedkiller, shaped like `/prune` with a `full` mode (whole repo) and a `pr` mode (the branch). Per comment:

- **Delete:** restating comments, docblocks that repeat the signature, and id and history comments.
- **Rewrite:** a comment whose fact is real but whose label is an id becomes the condition itself.
- **Check:** each kept why or invariant against the code. A wrong one is a finding, not a deletion.
- **Propose:** a test where an invariant can be pinned.
- **Park:** a surprise in our own code that needs a rename or a redesign becomes an idea (`docs idea add`). The cleanup never redesigns code.

Verify after each batch with the gate, as `/prune` does.

**Accepted cost.** A large one-time diff per repo. It is reviewed as its own feature, never folded into a build.

**Acceptance.** Run on factory itself, id-citing comment lines go from 692 to 0, the gate stays green, and the owner's sample read (§ QA plan) finds nothing lost that a reader would miss.

## 8. Decisions log

- **OPEN-1** — Invariants in our own code: keep them as a one-line comment, or require a test, type or rename and delete the comment (the `/no-comments` line)?
  - **Keep:** retains the class both audits say a reader would miss (races, coupled timeouts).
  - **Strict:** loses that class, because concurrency rarely renames away. In exchange, nothing in prose can go stale.
  - **Recommended:** keep, one line, and name the test that pins it when there is one.
- **OPEN-2** — Does the polish check block or only report?
  - Ids are mechanical, so they are Fix and polish clears them itself.
  - "Is it still true" needs a reader, so it stays a review question and only reports.
  - **Recommended:** exactly that split. No new owner question.
- **OPEN-3** — Is FR-5's skill in this PRD, or an idea for later?
  - **In:** it proves the rule on factory's own code, but makes the PRD bigger.
  - **Touched lines only:** each diff cleans the comments it touches, and there is no skill. It is cheap, but the untouched ids keep seeding copies for years.
  - **Later:** the 692 lines keep seeding copies in every factory run until then.
  - **Recommended:** in. Factory's own cleanup is its proof; cody and arca run it later as their own features.
- **OPEN-4** — Which doc comments count as a "public contract"? Are docblocks on private functions banned, or allowed when they state a contract?
  - Factory is a binary crate, so its `pub` items are not public API.
  - Laravel PHPDoc that PHPStan needs is types, not prose.
  - arca's new private functions carry a doc comment 64% of the time, and 17 of its 60 sampled comments repeat the signature.
  - **Recommended:** a library crate's pub API, an HTTP or CLI contract, and type annotations a checker reads. Every other docblock, private ones included, follows the rule for ordinary comments.
- **OPEN-5** — Ban `// FR-NN:` test headers and FR ids in test names?
  - **Cost:** grepping from an FR to its test stops working; the QA plan row is the mapping.
  - **Recommended:** ban them. In both field repos these headers spread fastest.
- **OPEN-6** — Should the cleanup also run on cody-agent and arca-api now?
  - **Recommended:** no, not in this PRD; each runs as its own feature once the skill ships.
  - The one exception is the wrong security comment in cody (`DynamicSetResolver.php:140`). It goes on cody's board now, not later.

## 9. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` *(declared in project.toml)*
**Surface**: the CLI (`factory comments check`), the worker packs, the skill texts.
**Harness**: the CLI test harness from PRD-005, `tests/harness/mod.rs`, with the stub worker `tests/stubs/claude`.
**Seed state**: a fixture branch with seeded comments: an id, a history note, a restating line, a real why and a real foreign quirk.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run the pack test: every role's pack holds `## Comments`. Then a live factory run on a small PRD: its retro numbers show comment share and id hits below PRD-005's | test transcript; `retro.md` numbers table | no |
| FR-2 | run `grep -rniE "in a comment\|code comment" skills/ assets/`: only the ban lines. `/slice` a small PRD: no slice file asks for a comment | grep transcript; the slice files read back | no |
| FR-3 | run `factory comments check --base main` on the seeded fixture branch: it lists the id and the history note, skips the why and the quirk, prints the share, exits 1 | CLI transcript | no |
| FR-4 | run a stub run to ship: `retro.md` holds the two new columns | `retro.md` read back | no |
| FR-5 | run `/comments full` on factory: id-citing comment lines 692 → 0, gate green | before/after counts; gate transcript | no |
| FR-5 | the owner reads 20 comments the cleanup kept and 20 it deleted, and judges whether a reader would miss any deleted one | owner's note in `retro.md` | yes |

## 10. Slices

_(Not sliced yet. Run `/slice`, or write "No slices — build straight from this document" if it is small.)_

## 11. Non-goals

- **Cleaning cody-agent and arca-api.** Each is its own feature in its own repo, run with FR-5's skill once it ships (OPEN-6).
- **Prose in `docs/`, READMEs, commit messages and PR bodies.** Narration belongs there.
- **Not handled: per-language comment linters** (clippy `missing_docs`, golangci `godot`, PHP-CS-Fixer comment rules). Accepted 2026-10-14: one tested verb covers every stack the owner uses. Revisit if the retro shows id hits creeping back past the check.

## 12. Verification notes

Checked against `feat/isolated-lanes` (b2fca3c) 2026-10-14. **Re-verify at slice entry.**

- `skills/`, `assets/`: `grep -i comment` hits only `assets/lookup.md:4` (`ctx refs` drops comments). No comment rule exists.
- `src/pack.rs:31` includes `assets/autonomy.md`; `src/pack.rs:236,293,377,484,569,620` push it into all six packs.
- `src/worker.rs:307`: `--append-system-prompt-file`. The pack is added to Claude Code's prompt and does not replace it, so the default "no comments, WHY only" rule reaches workers.
- `src/closeout.rs:2176-2179`: cites AD-1 and AD-30001 in a code comment.
- `src/checkpoint.rs:614`: "(FR-9)" means PRD-001's quota FR.
- `src/scheduler.rs:399-401`: the comment reads as the inverse of `checkpoint_waits = board.checkpoint_due && !free.is_empty()`.
- src+tests (Rust): 4,910 comment lines, 50,062 code lines, 692 comment lines citing an id, in 100 files. On `main`: 4,508 and 634.
- `src/retro.rs:162` `numbers()` builds the retro table between `factory:numbers` markers.
- `skills/slice/SKILL.md:83` (the trap line) and `:86` (every task traces to the PRD); `assets/contract-slice.md:17` (AD-n in the log), `:18` (the trap note).
- `skills/polish/SKILL.md`: § 1 Collect at `:29`, § 2 Verify at `:60`, § 3 Triage at `:78`, § 5 Fix at `:101`.
- `docs/features/004-skills/slice-01-spine.md:79`: "Write it in the doc comment."
- `src/db.rs:257-258`: "(slice 13)… before slice 13", a history note citing a slice.
- The field audits were read-only, 2026-10-13. cody-agent at d858bc4, arca-api at 91b4c59. Their file:line facts are theirs to re-verify.
