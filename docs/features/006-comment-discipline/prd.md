---
number: 006
slug: comment-discipline
title: "Comment and naming discipline"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-10-14
author: Emiliano Tisato
summary: |
  The plan stays in docs/. Code reads as if the plan never existed: no PRD, FR, AD, slice or checkpoint id in comments, names, paths, fixtures or strings, and a comment only for what the code cannot show. Four layers, because a prompt alone does not hold: one rule in every worker's pack, templates that stop modeling ids, one rule table (`factory leaks`) run at every checkpoint, in polish and in the doctor preflight, and factory's own cleanup, done and merged into `main` 2026-10-14 (`9e79a87`). The retro tracks the numbers and feeds missed classes back. **Interview done 2026-10-14, all seven OPENs settled. Adversarial pass done the same day** by a fresh agent on the session model (`factory review second` exited 2, cursor-agent unauthenticated): four wrong code claims, two contradictions, four detector rows that hit factory's own words, and six acceptance criteria with no surface, all corrected inline and dated. **The cut (2026-10-14) kept every extra but one**, the `note` on a three-line comment block; agreed and ready to slice from `main`. Research is in `research.md`.
---

# PRD-006 — Comment and naming discipline

Drafted from one session's research (2026-10-13/14): a 9-agent web and repo sweep with a skeptic per sweep (54 claims, 52 held), a comment audit of two field repos, and a second sweep for plan ids in names (§ 11 of the research). The full write-up is in [research.md](research.md). **The interview ran 2026-10-14**, three rounds; all seven OPENs are settled and every decision below carries its date. Factory's own cleanup (FR-5) ran the same day, and what it taught is folded into FR-1, FR-3 and FR-5. The adversarial pass ran after that; each correction is marked *Corrected 2026-10-14* where it sits.

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

It deletes everything except license headers, foreign quirks, public API contracts, issue links and justified lint suppressions. "When I am not sure a keep clause applies, the comment dies." Our own surprises get marked `MUST KILL` for a rename or refactor instead. FR-5's cleanup brief borrowed this shape.

Under that list, 8% of cody's sample survives and 18% of arca's. The keep-list is close to the right line. Factory changes three things:

1. **Keep invariants in our own code that no rename or type expresses**, like races, coupled timeouts and cross-module couplings (OPEN-1). The strict list deletes exactly the class both audits say a reader would miss. Examples: arca `internal/http/manage_apikey.go:159`, `internal/queue/jobs.go:381`; cody `PromptCompiler.php:237`.
2. **Allow links to external trackers only** (an upstream issue, an RFC). Internal ids rot.
3. **An unattended run parks a `MUST KILL` refactor as an idea** instead of redesigning code the PRD did not ask it to touch.

**The same leak in names (found 2026-10-14).** Plan ids also reach names and strings the code uses: test titles, fixture dirs, screenshot names, class and function names, test output. The worst cases are cody's browser titles, 61 of 196 carrying an FR id, and arca's drive titles, 56 of 130. Factory has a handful, including `tests/fixture/prd-004` and `src/db.rs:1226` `the_prd_003_migrate_body_runs_on_a_version_3_store`.

- **Factory removed this pattern once and it came back the same day.** 0d55577 deleted 3,145 lines of plan-named tests and frozen docs. That evening, PRD-005 created `tests/fixture/prd-004` (f35ed4c), because the owner's rule was written nowhere a worker reads.
- **No skill asks for id names, but factory's templates model them.** The QA plan and the slice's **Source** line key everything by FR (`assets/docs/slice.scaffold.md:19`). The harness template says "Name files by feature id" (`skills/harness/references/skill-template.md:72`).

Counts, spread, harm and seven options are in [research.md § 11](research.md). **This PRD covers names too** (OPEN-7, settled 2026-10-14): same cause, same rule, same check, same cleanup.

**Leaves alone:** prose in `docs/`, READMEs, commit messages and PR bodies; per-language linters; factory's own domain words (PRD, slice, checkpoint as concepts, not as a specific item).

## 2. Blast radius

- **Every worker in every repo on factory.**
  - `assets/autonomy.md` goes into all six packs (`src/pack.rs:236,293,377,484,569,620`).
  - Workers will write fewer comments, and polish will delete some they wrote.
  - The section costs tokens on every call; `assets/autonomy.md:62` says why that matters.
- **The skills `/slice`, `/prd`, `/polish`, `/gate` and `/harness`.** Their text changes. A skill edit means a version bump, a tag and `factory skills update` in each repo.
- **A new CLI verb, `factory leaks`, and four callers.** The checkpoint driver runs it after every green gate (`src/checkpoint.rs:125-147`); `factory docs doctor` (`src/docs/doctor.rs:59`) and `factory run`'s preflight (`src/docs/preflight.rs:98`) each print one line from it; `/polish` runs it in § 1 Collect. It breaks no existing contract.
- **The checkpoint worker's pack gains a `# Leaks` section** (`src/pack.rs:398-479`) and its contract one Do step (`assets/contract-checkpoint.md`). A checkpoint with hits does more work in its lane.
- **One output line of `factory docs migrate` changes** (`src/docs/migrate.rs:1699`): it stops printing "FR-13". The cleanup branch already does this.
- **The retro numbers table** (`src/retro.rs:162`) gains two columns, and the retro template one prompt line (`assets/retro.md:33-35`). Old retros keep their table.
- **Existing code is rewritten only in factory, by the cleanup (FR-5),** already done on `chore/comment-cleanup` for the owner to review and merge. Elsewhere the owner runs `factory leaks --all` and hands the report to an agent.
- **No external contracts.**

## 3. FR-1 — One rule every worker reads: the plan stays in docs/

**Today.** No factory asset or skill says anything about comments (`grep -i comment skills/ assets/`) or about names. Workers get only the default prompt and, on this machine, the output style. The owner's rule against plan-named tests was given in chat and written nowhere: the commit that applied it (0d55577) cites the owner's review `AD-9906` (`docs/features/002-docs-port/log.jsonl:82`), which is about tests reading this repo's docs and only names the test that went with them. PRD-005 broke the rule the same day. *Corrected 2026-10-14: an earlier draft said the log line held the naming rule.* The `assets/autonomy.md` sections are: Who decides, The test, Mapping, What an unattended run may write, Never to get green, Tokens, Caps, Blocker protocol, The owner answers mid-run, Resuming.

**Proposed (2026-10-14, the text the owner accepted).** Add this section to `assets/autonomy.md`, so every role reads it: build, fix, merge, checkpoint, polish and CI fix all write code.

```
## The plan stays in docs/
Code is read for years by people and agents who never saw the PRD. Write it so it
reads as if the plan never existed.
- Never name a specific plan item in code: not in file or folder names, classes,
  functions, tests, fixtures, strings or comments. No PRD-n, FR-n, AD-n, slice n,
  checkpoint n. Name the behavior: seedUnbilledWorkspaces, not seedPre025Workspaces.
- The QA plan maps each FR to its test. The test never names the FR.
- Comments: default to none. One short line only for what the code cannot show: an
  invariant no name or test expresses, a quirk of something we cannot change, or why
  a choice is deliberate. No history, no narration, no TODOs.
- A library crate's public API, an HTTP or CLI contract, and type annotations a
  checker reads keep a doc comment. Every other docblock follows the line above.
- Do not copy what is nearby when it breaks this. Fix it when you touch it.
```

- **One principle, not a list of bans.** "Reads as if the plan never existed" covers cases a list would miss. The full id list lives in FR-3's check, not in the prompt.
- **It bans specific plan items, not the words.** In factory, PRD, slice and checkpoint are the product's own concepts: `struct Slice`, `parse_prd` and `Framework::V3` stay.
- **Own-code invariants keep their one line** (OPEN-1, settled). This is looser than `/no-comments`, which deletes them.
- **Contracts keep a doc comment** (OPEN-4, settled 2026-10-14, "contracts only"). In factory that is the clap `///` help text and nothing else: a `pub fn` in a binary nobody imports is not a contract, and `src/lib.rs` exists so `tests/` can link. Private docblocks follow the ordinary rule, which is what deletes the signature-repeating docblocks on 64% of arca's new private functions.
- **Two readings settled by the cleanup (FR-5, 2026-10-14), without growing the text.** `// SAFETY:` on an `unsafe` block is the invariant the block relies on, so it stays, one line; where clippy's `undocumented_unsafe_blocks` is on, a checker reads it too. A one-line grammar above a hand parser of an external format is "what the code cannot show". A multi-line header that is the format reference of a stub's scenario file is the same, and the only block over one line factory keeps on purpose. *Corrected 2026-10-14: an earlier draft said a checker reads the SAFETY line in factory. None does (`Cargo.toml` has no `[lints]`), and the cleanup had deleted one of the four (`src/review.rs:233`); db7ae02 restores it.*

**Why a prompt alone does not hold.** Workers already get two no-comment rules, and 14% of factory's comment lines still cite an id. The rule is still needed: the `prd-004` fixture came back partly because no worker could read the owner's rule. FR-2 to FR-5 back it with inputs that stop modeling ids, a check and a cleanup.

**Where it lives.** The research synthesis put a comment rule in `assets/contract-slice.md`, next to the trap line (`:18`). This draft puts it in `autonomy.md` instead, for the reason under "Alternative that lost".

**Accepted cost.** About 230 tokens on every call of every worker. We take it because one wrong comment costs more: about 24% accuracy for the next reader.

**Alternative that lost.** Putting it in `contract-slice.md` only. Fix, merge and polish workers write code too, and the comment at `src/closeout.rs:2176` came from a polish run.

**Acceptance.** Every role's pack holds the section; a unit test over `pack.rs` checks it. That is the build's proof. The rest is observed after ship, on the next real run: its retro columns (FR-4) come in below PRD-005's added lines, 12.0% of non-blank lines as comments and 14.7% of those citing an id (`git diff 8458b30 611dcd1 -- src tests`, added lines, FR-3's first-row pattern, any case), and it adds no name with a plan id (PRD-005 added one). The baseline lives here because earlier retros have no such columns. *Corrected 2026-10-14: an earlier draft made the live run a build proof; a stub run writes no real comments.*

## 4. FR-2 — Specs and templates stop modeling comments and ids in code

**Today.** Agents follow what they read more than what they are told, and factory's inputs teach both habits:

- **Comments.** The field repos' slice briefs ask for them ("state the invariant in the comment", arca `012/slice-03-one-scheduler.md:222`). The densest feature has the most such asks. `/slice`'s "trap" lines (`skills/slice/SKILL.md:83`) are written for the builder, and builders copy them into code.
- **Ids.** No instruction asks for id names, but the templates make the FR the unit of proof:
  - The QA plan keys each proof row by FR (`assets/docs/prd.scaffold.md:41-43`), and the proof row "names the test or command" (`skills/prd/SKILL.md:134`). Test names drift toward the row id.
  - The harness template lists the trigger "prove FR-n" (`skills/harness/references/skill-template.md:12`) and says "Name files by feature id" (`:72`). Its examples are behavior names (`rules-create.png`), but in factory "feature id" reads as a plan id.
  - Four lines of shipped skill text cite factory's own ids bare, and they read wrong in another repo, where PRD-001 is a different feature: `skills/prd/SKILL.md:66`, `skills/gate/SKILL.md:132`, `:134` and `skills/harness/SKILL.md:33`.

**Proposed (2026-10-14).**

- **`/slice` and `/prd` never ask for a code comment.** An invariant the slice must keep becomes a test that pins it, or a type or a name. The trap is for the builder, not the code.
- **The QA plan is the only map from FR to test** (settled with the rule, 2026-10-14). The proof row names the test by its behavior name, and the test never names the FR. `skills/prd/SKILL.md:134` says so.
- **The harness template stops modeling ids.** `:12`'s trigger becomes "prove a § QA plan row". `:72` becomes "Name files by the behavior", which its examples already do.
- **The four anecdotes state the fact without the id,** or name the repo the way `skills/gate/SKILL.md:63` does ("cody-agent's PRD-011").
- **Ids stay where they belong.** The slice's **Source** line (`assets/docs/slice.scaffold.md:19`), the PR body table (`assets/pr-body.md:30`), the log's AD-n and the format examples (`FR-1` in the scaffold, `PRD-013 → 5 slices` in `/slice`) are docs, so they keep their ids.

**Accepted cost.** `grep FR-12 tests/`, `--filter=FR-157` and `-g 'FR-61 pin'` stop finding the test. The QA plan row names it instead, and a bare id is ambiguous anyway (arca's OPEN-5, cody's AD-12). One skill release.

**Acceptance.** `grep -rniE "in a comment|code comment" skills/ assets/` finds nothing, as today (FR-1's text uses neither phrase); the point is that the skill edits add no ask. The four anecdote lines and the two harness-template lines read back without a factory id. A small PRD sliced after the change has no slice file asking for a comment or naming a test after an FR (read back).

## 5. FR-3 — `factory leaks`: one rule table, run at the checkpoint, in polish and in the preflight

**Today.** `/polish` has no comment or naming step (`skills/polish/SKILL.md`), and nothing measures either. The checkpoint worker's pack holds `# Gate transcript`, `# Covered slices`, `# QA rows` and `# Log` (`src/pack.rs:398-479`), and its contract (`assets/contract-checkpoint.md`) has it prove rows and fix what fails. `factory docs doctor` prints Framework, gate, skills and drift (`src/docs/doctor.rs:59`); `/prd` § 0 and `factory run` read it.

**Proposed (2026-10-14, settled in the interview).**

- **A verb, `factory leaks [--base <ref> | --all]`, built as a rule runner.** One embedded table of rules. Each rule is an id, a detector over added lines or paths, a severity and a one-line fix instruction. The report groups hits by rule and prints the rule's fix line above them, so one text serves the checkpoint worker, the polish worker, the doctor line and a human who pastes it to an agent. A new class of leak is one row in the table and nothing else changes. That is the owner's ask (2026-10-14): "tomorrow I may find something new and we have an easy place to put it and the machinery already will run it."
- **The input.** `--base <ref>` reads `git diff -U0 <base>...HEAD` outside `docs/`: the added lines and the added paths. `--all` reads the whole tree against the empty tree, for adoption and for the doctor line. An extension table tells comments from code: rs, go, php, js/ts/vue, py, sh, toml/yaml, json/jsonl. Markdown outside `docs/` is skipped, like READMEs.
- **The rules at ship.** A `fix` hit exits 1; a figure is never a hit. (A `note` severity, one row for a comment block of three or more lines, was cut at the cut question, 2026-10-14; see § Non-goals.)

  | Rule | Detector | Severity |
  |---|---|---|
  | id in a comment | `(PRD\|FR\|AD\|CP\|OPEN\|OQ\|DEC\|BLOCK\|ASK)-\d+`, `slice[- ]?\d+` any case, `§` | fix |
  | id in a name or path | the same items in identifiers, test titles and file or folder names, any case, joined by `-`, `_` or nothing: `prd_003`, `tests/fixture/prd-004`, `frNNN-` screenshots, `slice2Allowlists`, `section_14` | fix |
  | id in a string literal | the same, outside the allowlist: `'FR-157: Support switches…'`, `cp1-fr43-…` | fix |
  | history words in a comment | "until slice", "for now", "previously", "no longer", "added in", "used to", "not built yet", "a later slice", "before PRD", "since slice", "as corrected", "confirmed on". Comments only: in a string, "no longer" is prose to a user (`src/scheduler.rs:2015`) | fix |
  | a line cite into a file | `\w[\w.-]*\.(md\|mjs\|js\|ts\|rs\|go\|php\|py):\d+(-\d+)?` in a comment or string: `docs.mjs:1930`, `autonomy.md:78`, `measure.py:42-67` | fix |
  | a skill or asset section cite | `/\w+ §`, `\w+\.md §`, `(PRD §`, `prd §`: `/deliver §3`, `autonomy.md § Caps`. Not `SKILL.md` alone: it is factory's own file name, on 50 string lines | fix |
  | a task or step marker | `\b[Tt]asks? \d`, `/// Task \d+:`, a banner `── (task \d+\|PRD-\|FR-\|slice \d)`. Not a bare phase word: closeout's own `── polish ──` banner (`src/closeout.rs:132`) | fix |
  | a review-finding id in a comment | `\b(C\|Cn\|K\|L\|Ln\|S\|X[a-d])\d{1,2}\b`, `finding [A-Z]+\d`: `C3:`, `Cn4`, `K3`, `L11`, `Ln6`, `S1` (tuned: `L1` is also a cache) | fix |
  | plan words with no id, in a comment | "the spec says", "the PRD says", "qa-guide row", "slice-entry", "checkpoint's transcript" | fix |
  | comment share of the added lines | | figure |

  The last five rows and the longer history list came from FR-5's cleanup (2026-10-14); its learnings say what each one caught. A sixth, a date in a comment, was dropped after the adversarial pass: both dates the cleanup found sat beside a history word, and the two left on the branch (`src/measure.rs:282`, `tests/harness/mod.rs:83`) explain a fixture.
- **An allowlist in `project.toml`, because factory uses ids as data.** Factory numbers BLOCK-n, AD-n and CP-n itself, so about 400 non-comment Rust lines hold id-shaped strings today (`tests/lanes.rs` alone has 81). A path allowlist. An entry carries a scope: `strings` (the default) turns off the string check under the path and nothing else; `all` turns off every rule there, comments, names and paths included. `all` is for a test file that narrates its fixture's scenario by the fixture's own slice numbers (eight lines in four files on the branch: `tests/checkpoints.rs:481,658,743,1368,1422`, `tests/closeout.rs:937`, `tests/merge.rs:96`, `tests/quota.rs:377`), for recorded output and fixtures (`tests/stubs/cursor-review.jsonl`, `tests/fixture/v2/`) and, in another repo, for the migration directory whose file names the database records. An `all` entry hides a future leak under it, so the list stays that short and polish's reader question is what still covers those files. *Corrected 2026-10-14: an earlier draft said names and paths are always checked and counted ten lines in five files; the row's own pattern found thirteen, five of them rewritten in db7ae02, and a migration path with an id needs `all`.*
- **Where it runs** (OPEN-2, settled 2026-10-14: checkpoint and polish, with the report as the worker's prompt).
  1. **At every checkpoint.** After the gate goes green (`src/checkpoint.rs:125-147`), the driver runs `factory leaks --base <feature base>` and puts the report into the checkpoint worker's pack as a `# Leaks` section, after `# Gate transcript`. The contract gains one Do step: fix every `fix` hit in the file it names, verify with the inner-loop command, and list them under **Fixed**. The driver commits the lane as it does today. A hit the worker could not fix is named under **Fixed** as unfixed, and the next checkpoint shows it again.
  2. **In polish.** `/polish` runs it over the whole branch in § 1 Collect (`skills/polish/SKILL.md:29`); hits pass § 2 Verify like any reviewer's and land in § 3's Fix bucket, with no question to the owner. Its code review gets one question per surviving added comment, security claims first: does it still match the code beside it, does it sit on the item it describes, does it name only what the tree defines, and would a reader miss it? And one per added name: does it point at the plan without a number, like `weights_match_the_prd_formula` or cody's `seedPre025Workspaces`? A reader catches what the table cannot: `scheduler.rs:399` (the inverse of its flag), cody's false security claim, the three docblocks FR-5 found on the wrong item, and a comment naming a ported tool's function (`allocateNumber`).
  3. **In the preflight.** `factory docs doctor` prints one more line from `--all` on the tree: `leaks    N hits — run factory leaks --all and clean before the first run`, or `leaks    clean`, and `factory run`'s preflight (`src/docs/preflight.rs:98`) prints the same line from the same function. It warns and never blocks, like drift. `/prd`, `/slice` and `/lanes` open with `factory docs doctor`, and every run opens with its preflight, so an adopting repo sees the line before any worker writes code, and the owner cleans first. The spread data is the reason: a rule shipped into a repo with 552 old ids loses to copying. *Corrected 2026-10-14: an earlier draft said every preflight reads doctor. `doctor::run` has one caller, the `doctor` verb (`src/cli/docs.rs:308`); `factory run` builds its own verdict in `preflight::run` and never calls it, and `docs init`, `docs migrate`, `upgrade` and `skills update` call neither. Hence the second print.*
  4. **At ship.** FR-4.
- **Mechanical hits are fixed by the worker; judgment stays with a reader.** Ids and history words are `fix`. "Is it still true" and "does this name point at the plan" are review questions. No new owner question anywhere.

**Accepted cost.** A new verb with a rule table, a comment-syntax table per language, a tuned name pattern, one more pack section and one Do step for the checkpoint worker, one doctor line that scans the tree on every call, and a longer checkpoint when there are hits. Short forms collide with real names: Go's `slice1`, the `cp1252` codepage, and factory's own test locals that hold a fixture's log entries (`ad1`, `ad2` in `src/docs/log.rs` and `src/docs/check.rs`). So the pattern is tuned against all three repos before it ships.

**Alternatives that lost.**

- A grep written into the skill text: no unit test, and each repo drifts its own copy.
- Fixing leaks only in polish: the ids sit in the tree through every slice and seed copies, while the checkpoint already has a worker in a lane with the gate green.
- Refusing to run while the base branch has leaks: a hard stop for cody and arca on the day they upgrade, and a baseline key in `project.toml`.
- A flag under `/prune`: prune is tests-only (`skills/prune/SKILL.md:12`) and runs in polish `pr` mode, while leaks must run at the checkpoint and over a whole tree.

**Acceptance.**

- On a fixture branch with seeded lines, `factory leaks --base main` lists the planted leaks under their rule, each under its fix line, and exits 1. The planted leaks are an id comment, an "until slice 4", a test named after an FR, and a fixture folder named `prd-9`. It leaves the real why, the foreign quirk and an allowlisted `BLOCK-1` alone, and prints the share.
- A stub run with those leaks seeded in a slice: the checkpoint pack holds `# Leaks` with the hits, the stub worker's scenario fixes them and names them under **Fixed**, and polish's run reports zero hits.
- `factory docs doctor` and `factory run`'s preflight on that branch print the `leaks` line with the count; on the feature branch, which holds the cleanup (merged before slicing), they print `clean`.
- A read-only `--all` run over the whole of cody-agent (`~/Projects/CODESM/cody-agent`) and arca-api (`~/Projects/Prokey/arca-api`), and over the feature branch, reports no false hit: the builder writes every hit and its verdict into the slice's log entry, and a reader agrees each one is a real leak.

**Changed 2026-10-14, at the owner's review.** The run tuned "used to" as purpose, "previously" as an adjective and "for now and" out of the `history` row (AD-4). The owner reversed it: they are hits again, about 60 false ones over cody and arca, taken over a history comment in those shapes passing. The no-false-hit line above holds for every other row.

## 6. FR-4 — The retro tracks the numbers and feeds missed classes back

**Today.** The retro's numbers table (`src/retro.rs:162`, between `<!-- factory:numbers -->` markers) has tokens, slices and tokens per slice, but no comment figures. The retro template has a `## Framework friction → skills repo` section (`assets/retro.md:33-35`), and closeout files that section as an idea in the `[source]` checkout when it differs from the template (`src/closeout.rs:356-373`, PRD-004 FR-7).

**Proposed (2026-10-14).**

- **Two columns** in the numbers table, filled from `factory leaks --base` at ship time: the added comment share and the id hits in comments, names and strings. They are compared with the three earlier runs, as the table already does. This is how the owner sees whether FR-1 and FR-2 work, run over run.
- **One prompt line in `## Framework friction → skills repo`:** "A comment or name the leaks check should have caught: `file:line` and the line." Closeout already files that section here as an idea, so a missed class reaches this repo's board, and the owner adds a row to FR-3's table when they agree. Evidence only, with `file:line`: the template's own warning applies, an agent asked for candidates invents them.

**Accepted cost.** One more measurement at ship, a wider table, one template line.

**Alternative that lost.** A new `## Leak candidates` retro section. A question on every run invites invented candidates and needs its own filing path. The numbers show the trend; the friction line records the miss.

**Acceptance.** A stub run to ship writes a `retro.md` whose numbers table holds both columns and whose friction section carries the new line. With that line filled in, closeout files it as an idea in the `[source]` checkout.

## 7. FR-5 — Factory cleans itself, and the learnings shape the check

**Today (before the cleanup).** The weeds were already planted: 692 id-citing comment lines in factory (`main` at c913d36, src+tests, counted as below), 552 bare ids in arca's Go code, the cody tags above, and the names in research § 11. Because agents copy what is nearby, a rule that only applies going forward keeps losing to them. Renames alone do not hold either: cody's 22 renames (aaf6706) came back within two weeks, and factory's 0d55577 the same day, both with no rule a worker reads.

**Done 2026-10-14, before slicing.** The owner chose to clean now, by agents on a branch, over two prompts to drive by hand and over a slice in the build (interview round 3). Eleven agents, one per module group, applied FR-1's rule to `src/` and `tests/` on `chore/comment-cleanup`, with no cargo between them and the gate once at the end: fmt, clippy and 570 tests green, one commit (`cecfc3f`), then two small ones after the adversarial pass (`a264b99`, `db7ae02`), gate green each time. Each agent read the FR-1 text plus a ten-step order per comment: a plan cite → drop it and keep the fact as one line when the code cannot show it; restates → delete; history → delete; argues correctness → delete; TODO → delete and report; what the code cannot show → keep, one line, 20 words; a plan-named test → rename; a plan-citing string → rewrite after a grep of `tests/` and `src/`; a wrong comment → delete, never patch; a banner → keep only without a plan id. The owner reviews the branch by sample and merges it; merging is theirs.

**The numbers** (src+tests Rust, `main` c913d36 against the branch at db7ae02, `grep -cE '^\s*//'` and FR-3's first-row pattern, any case):

| | before | after |
|---|---|---|
| lines | 58,426 | 54,565 |
| comment lines | 4,910 | 1,057 |
| comment lines citing an id | 692 | 8 |
| `///` lines | 3,039 | 571 |
| `//!` lines | 764 | 66 |

108 files, 842 insertions, 4,769 deletions. The eight id lines left name the test fixture's own slices (`tests/checkpoints.rs:658` "Slice 03 touches docs only", in four test files): they are data, and FR-3's allowlist covers them as `all`. One clap help line (`src/cli/docs.rs:128`) showed an origin value as `PRD-016 polish`; it shows the format, `PRD-NNN polish`, instead, and a docblock that showed a subject as `feat(018): slice-02 Harness` (`src/commit.rs:148`) shows `feat(NNN): slice-NN Title` for the same reason. *Corrected 2026-10-14: the first count of what was left used a narrower pattern than FR-3's row and said ten lines in five files. The row's own pattern found thirteen: the eight above, four block and ask ids in test comments (`src/cockpit.rs:589`, `tests/lanes.rs:771,968`, `tests/loop.rs:1096`) and the subject example, all five rewritten in db7ae02. The same commit restores the one `// SAFETY:` line the cleanup had deleted (`src/review.rs:233`).* Six test functions and one fixture folder were renamed (`src/db.rs`, `src/quota.rs`, `src/cockpit.rs`, `src/cli/report.rs`, `tests/stubs.rs`, `src/ctx/refs.rs`; `tests/fixture/prd-004/` → `tests/fixture/log-without-covers/`). Ten strings were rewritten: the migrate output line, four block texts in `src/closeout.rs` and one in `src/scheduler.rs` that cited `autonomy.md` by line or section, two stub messages, the gh stub's PR title and the one assert that matched a stub message. `///` blocks on items in `src/` went from about 950 to about 310, 40 of them clap help. About fifteen comments were wrong or sat on the wrong item and were deleted, not patched: `src/closeout.rs:2176` (AD-1 and AD-30001 for facts those decisions do not cover), `src/scheduler.rs:399` (the inverse of its flag), four event and gate docs in `src/events.rs` and `src/db.rs` that named an emit condition the code does not have, three docblocks that described the next function. No TODO existed. Four stub headers (`tests/stubs/claude`, `gh`, `cursor-agent`, `tests/fixture/gate.sh`) stay as multi-line format references for their scenario files: the only comment blocks over one line kept on purpose.

**What the cleanup taught**, folded in the same day:

- **FR-3's table gained five rows** for the classes the first table missed: a `file:line` cite into a source, asset or skill file (`docs.mjs:1930`, `autonomy.md:78`, `measure.py:42-67`), a skill or asset section cite (`/deliver §3`, `autonomy.md § Caps`), a task or step marker (`task 2`, `// ── task 3 (a):`), a review-finding id (`C3`, `Cn4`, `K3`, `L11`, `Ln6`, `S1`), and plan words with no id (`the spec says`, `qa-guide row`, `slice-entry`). The history row gained `not built yet`, `a later slice`, `before PRD`, `since slice` and `confirmed on`. A date row was tried and dropped: both dates found (`as corrected 2026-10-09`, `confirmed on`) sat beside a history word. Two detectors were cut back when the adversarial pass ran them over factory's own tree: `SKILL.md` alone and the `── polish` banner are the product's own words. Two shapes no grep finds went to polish's review questions: a comment that names something the tree does not define (`allocateNumber`, a ported tool's name) and a docblock on the wrong item (three found).
- **The allowlist covers comments, names and paths too, per path.** A comment that narrates a fixture scenario by the fixture's own ids is data, and the first table said comments are always checked. An entry now carries a scope, `strings` (the default) or `all`; factory lists its four scenario test files and its recorded fixtures as `all`. Cost: an `all` entry hides a future leak under it, and polish's reader question is what still covers it.
- **The rule text holds; two clauses were read into it, not added.** `// SAFETY:` on an `unsafe` block is the invariant the block relies on, so it stays, one line; factory has four, and the one the agents dropped is back. A one-line grammar above a hand parser of an external format is "what the code cannot show". Both are FR-1 notes now.
- **Ids as data are decided by location, not by shape.** A string compared with the tool's output or a fixture is data; the same id in a comment, an identifier or a path is a leak, unless the comment narrates the fixture (the allowlist case). Factory's own PRD numbers collide with the test fixture's (`PRD-001` is both), so the table's `fix` hits are the unambiguous shapes and a reader decides the rest.
- **False positives the tuning keeps out:** asset names (`the_prd_scaffold_frontmatter_has_the_seven_keys` names `prd.scaffold.md`), schema and store versions (`v2`, `version_3_store`), gate kinds (`"checkpoint"`, `"slice"`), `PRD-{feature}` format strings, a skill's own section number inside a string (polish's `§5`), and the fixture's slice numbers in test comments.
- **The dominant deletion was not a plan leak.** Most of the 3,800 lines were docblocks restating a name or a signature (`/// One runs row.` on `RunRow`; a prose rendering of the `gh` command a function runs). FR-1's "default to none" line is what deletes them, the rule needs no clause for them, and FR-3's comment-share figure is what shows them creeping back.

**No cleanup skill** (OPEN-6). The brief above was the one-time prompt; from FR-3 on, the report of `factory leaks --all` is the prompt, in factory and elsewhere.

**Never renamed:** migration files, because the database records their names; a migration directory sits on FR-3's allowlist as `all`. In factory, recordings of real output stay as recorded and sit on the allowlist the same way.

**Accepted cost.** One large diff reviewed as its own branch, by sample: 108 files at once, with no slice-sized commits to read. Every `file:line` in this PRD and in PRD-005's slices shifted; § Verification notes carries the new ones.

**Acceptance.** As § QA plan says. The owner's sample read of 20 kept comments, 20 deleted ones and the renames happens before the merge, and the merge of `chore/comment-cleanup` into `main` is the verdict: anything missed becomes a dated line in this section. The merge comes before `/slice`, so the feature branch holds the cleanup and the run's proof is `factory leaks --all` there: zero `fix` hits outside the allowlist, `factory docs doctor` printing `leaks    clean`, the gate green (it is, on the branch). *Corrected 2026-10-14: an earlier draft had the run prove this on `main` after a merge that only the owner can do, which put the owner mid-build.*

**Merged 2026-10-14** as `9e79a87`, on the owner's word and before the sample read; the gate is green on `main` (570 tests). The read now runs on the merge's diff, `git diff 9e79a87^1 9e79a87`, and a miss becomes a dated line here.

**Sample read 2026-10-14.** The owner's read: 20 kept comments pass, 7 renames pass; of 20 deleted, 3 lost a why: `src/cli/review.rs:1-2` (the exit contract), `src/closeout.rs:694` on `block_conflict` (only the owner moves the tip; the block keeps the commit), `tests/closeout.rs:818` on `stood_in_run` (which String is which). All three are restored in the commit that adds this line. Borderline, left as is: the `tests/merge.rs` pointer kept in `tests/commits.rs`, and the deleted "this repo's copy" on `PR_BODY` in `src/pack.rs`. The class, a long why squeezed to one line that drops its contract, has no grep, so it gets no FR-3 row.

## 8. Decisions log

- ~~**OPEN-1** — Invariants in our own code: keep them as a one-line comment, or require a test, type or rename and delete the comment (the `/no-comments` line)?~~ **Settled 2026-10-14:** keep, one line, in FR-1's rule. Strict would lose the class both audits say a reader would miss (races, coupled timeouts), because concurrency rarely renames away.
- ~~**OPEN-2** — Does the polish check block or only report?~~ **Settled 2026-10-14:** checkpoint and polish. The driver runs the check after every green gate and the report goes into the checkpoint worker's pack with a fix line per rule; polish runs it again over the branch. Ids and history words are fixed by the worker, "is it still true" and "does this name point at the plan" stay review questions. No new owner question. The owner's first answer asked for the mechanical report to carry its own prompt for the agent; that is the fix line.
- ~~**OPEN-3** — Is FR-5's skill in this PRD, or an idea for later?~~ **Settled 2026-10-14:** in, as the fourth layer, and done the same day by agents on a branch rather than as a skill (OPEN-6): the untouched ids keep seeding copies. FR-1's "Fix it when you touch it" also cleans touched lines from then on. cody and arca get the `factory leaks --all` report later, when the owner chooses.
- ~~**OPEN-4** — Which doc comments count as a "public contract"? Are docblocks on private functions banned, or allowed when they state a contract?~~ **Settled 2026-10-14:** contracts only. A library crate's pub API, an HTTP or CLI contract, and type annotations a checker reads keep a doc comment; every other docblock, private ones included, follows the ordinary rule. Factory is a binary crate, so its `pub` items are not public API and only the clap help text is a contract. Laravel PHPDoc that PHPStan needs is types, not prose. FR-1's rule gained the line.
- ~~**OPEN-5** — Ban `// FR-NN:` test headers and FR ids in test names?~~ **Settled 2026-10-14:** banned, in FR-1's rule: "The test never names the FR". The QA plan is the only map from FR to test (research § 11, option C), so `--filter=FR-157` stops working.
- ~~**OPEN-6** — Should the cleanup also run on cody-agent and arca-api now?~~ **Settled 2026-10-14:** no, and there is no cleanup skill. The owner's words: a mechanism that runs once per repo and never again is not worth building; the report of `factory leaks --all` is the prompt, and the owner hands it to an agent in cody and arca when they choose. Factory itself was cleaned the same day (FR-5). The wrong security comment in cody (`DynamicSetResolver.php:140`) is already on cody's board (`docs/ideas.md`, parked 2026-09-06) and needs a fix there, not here.
- ~~**OPEN-7** — Does this PRD cover plan ids in names too (tests, fixtures, classes, strings), or only comments?~~ **Settled 2026-10-14:** same PRD. Same root cause, one rule (FR-1), one set of template edits (FR-2), one check (FR-3) and one cleanup (FR-5). The research's options A to F are in; G (rename on touch only) is the rule's last line, on top of the cleanup. E (`/prune`) is not needed.

**Also settled in the interview (2026-10-14), none of them an OPEN:**

- **The verb is `factory leaks`,** over `factory lint` and a flag under `/prune`.
- **The check is a rule table,** so a new class is one row (FR-3). A per-repo table is not built (§ Non-goals).
- **The adoption preflight is one doctor line,** over refusing to run on a leaky base and over nothing at adoption (FR-3).
- **The retro feeds missed classes back through the friction section it already has,** over a new section and over numbers only (FR-4).
- **Factory's cleanup ran at once, by agents on a branch,** over two prompts for the owner to drive and over a slice in the build (FR-5). The owner reviews by sample and merges.

## 9. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` *(declared in project.toml)*
**Surface**: the CLI (`factory leaks`), the worker packs, the skill texts.
**Harness**: the CLI test harness, `tests/harness/mod.rs` (PRD-001's, grown since), with the stub worker `tests/stubs/claude`.
**Seed state**: a fixture branch with seeded lines: an id comment, a history note, a real why, a real foreign quirk, a test named after an FR, a fixture folder named `prd-9`, and an allowlisted `BLOCK-1` string. (A restating line is a reader's call, not the verb's, so none is seeded.) **Precondition**: `chore/comment-cleanup` merged into `main` before `/slice`, so the feature branch holds it.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run the pack test: every role's pack holds `## The plan stays in docs/`. The live-run numbers are observed after ship, on the next real run's retro (FR-4), against the baseline in FR-1 | test transcript | no |
| FR-2 | run `grep -rniE "in a comment\|code comment" skills/ assets/`: nothing, as today. Read back the four anecdote lines and the two harness-template lines. `/slice` a small PRD: no slice file asks for a comment or names a test after an FR | grep transcript; lines and slice files read back | no |
| FR-3 | run `factory leaks --base main` on the seeded fixture branch: it lists the id comment, the history note, the FR test name and the `prd-9` folder under their rules, each under its fix line, skips the why, the quirk and the allowlisted string, prints the share, exits 1. Then run it read-only with `--all` over `~/Projects/CODESM/cody-agent`, `~/Projects/Prokey/arca-api` and the feature branch, and write every hit with its verdict into the slice's log entry | CLI transcripts; the log entry | no |
| FR-3 | run a stub run with the seeded leaks in a slice: the checkpoint pack holds `# Leaks` with the hits, the stub worker's **Fixed** names them, polish's run reports zero. Run `factory docs doctor` and `factory docs preflight` on the seeded branch: the `leaks` line shows the count | pack and transcript read back; doctor and preflight output | no |
| FR-4 | run a stub run to ship: `retro.md` holds the two new columns and the friction line; with the line filled, closeout files an idea in the `[source]` checkout | `retro.md` and the idea file read back | no |
| FR-5 | run `factory leaks --all` on the feature branch, which holds the merged cleanup: zero `fix` hits outside the allowlist, and `factory docs doctor` prints `leaks    clean`; the gate is green | CLI and gate transcripts | no |
| FR-5 | the owner reads, on the merge's diff (`git diff 9e79a87^1 9e79a87`), 20 comments the cleanup kept, 20 it deleted and the renames; a miss becomes a dated line in FR-5 | FR-5 | yes |

## 10. Slices

Five slices on `feat/comment-discipline`: one branch, one PR, one commit per slice, Prove at the checkpoints. `factory docs status` is the board.

- **01, the `factory leaks` verb** (FR-3: the rule table, the input, the allowlist, the verb). The scan is a library function that returns the hits and the two figures, so the checkpoint, the doctor line and the retro call it without a process. The spine: 03, 04 and 05 call it.
- **02, the rule, the skill texts and 0.4.0** (FR-1, FR-2, and FR-3's polish call). The FR-1 section in `assets/autonomy.md` and its pack test, the slice, PRD and harness-template edits, the four anecdotes, polish's § 1 Collect, and the one version bump. Text and one test, no code 01 touches.
- **03, ⚠ the highest risk: tuning, the allowlist, doctor and preflight** (FR-3's tuning and "Where it runs" item 3, FR-5's run row). The detectors meet three real trees: Go's `slice1`, `cp1252` and factory's `ad1` read like leaks by shape alone. Factory's own allowlist and its two test-module renames land here.
- **04, leaks at the checkpoint** (FR-3's "Where it runs" item 1). The driver scans after a green gate, the pack carries `# Leaks`, the contract gains its Do step, and the stub gains a key that fixes the hits.
- **05, retro columns and the friction line** (FR-4). Ship stores the comment share and the id hits, the compare table shows both, and the template's friction section gains its prompt line.

**Order.** 01 goes first because every later code slice calls its `scan`. 02 depends on nothing and runs beside it. 03, 04 and 05 each depend on 01 only, and run three wide after it: they touch different files, except two hunks of `src/closeout.rs` (04 makes `default_branch` visible to the checkpoint, 05 edits ship). Width 2, then 3, within `lanes = 4`.

**Checkpoints** (`checkpoint_every = 2`). The first covers 01 and 02, so the verb and the rule are proven before three slices build on the verb. The second covers the first two of 03 to 05 to finish; the last covers the third. FR-5's run row belongs to 03, and 04 and 05 name it too, so the final checkpoint reruns it on the tree that ships whichever lands last.

**The installed 0.3.0 driver builds this run**, and it predates every part of this feature. Its checkpoints scan nothing, its workers' packs lack FR-1's section (the autonomy text is compiled in), and polish's `factory leaks` call exits 2, which slice 02's polish text says out loud as "did not run". The checkpoints run the verb from the lane's build instead (`cargo run -q -- leaks --all`), so FR-5's row is what catches a leak a builder writes in this run. Once 03 lands `[leaks]` in `docs/project.toml`, the old driver reads it as an unknown key: a warning, not an error (`src/config.rs:186`). 0.4.0 is tagged after the merge, not in the run.

**[`qa-guide.md`](qa-guide.md)** holds the one Human row: FR-5's sample read of the cleanup merge.

## 11. Non-goals

- **Cleaning cody-agent and arca-api.** The owner runs `factory leaks --all` there and hands the report to an agent, when they choose (OPEN-6). That includes their harness skills' feature maps, keyed by FR today (cody's `verify-cody-agent` carries about 130 plan mentions).
- **A cleanup skill.** Decided against 2026-10-14 (OPEN-6): the report is the prompt, and a mechanism that runs once per repo is not worth a skill.
- **Prose in `docs/`, READMEs, commit messages and PR bodies.** Narration and ids belong there.
- **Factory's domain words.** PRD, slice, checkpoint, `Framework::V3` and `LEGACY_DOC_DIRS` name the product's concepts, not a plan item.
- **Renaming migrations.** The database records their names; the directory goes on FR-3's allowlist as `all`.
- **Not handled: per-language comment linters** (clippy `missing_docs`, golangci `godot`, PHP-CS-Fixer comment rules). Accepted 2026-10-14: one tested verb covers every stack the owner uses. Revisit if the retro shows id hits creeping back past the check.
- **Not handled: a per-repo rule table** (rules in `project.toml`). Accepted 2026-10-14: every rule so far is a framework rule, and a repo-only rule would drift. Revisit when a repo needs a rule factory's table should not carry.
- **Not handled: refusing to run on a leaky base.** Accepted 2026-10-14: the doctor line warns and the owner cleans. Revisit if a repo adopts the rule and its id hits do not fall within two features.
- **Not handled: a `## Leak candidates` retro section.** Accepted 2026-10-14: the friction line and the numbers are enough. Revisit if a missed class shows up in the numbers and nobody filed it.
- **Not handled: a `note` on a comment block of three or more lines.** Cut 2026-10-14 at the cut question: no evidence beyond the comment-share figure, which already shows a docblock growing back. Revisit if the share figure climbs across runs while the id hits stay at zero.

## 12. Verification notes

Checked against `feat/isolated-lanes` (b2fca3c) 2026-10-14; the naming lines against `main` (30123ca) the same day; the checkpoint, doctor and retro lines against `main` (c913d36) the same day. The `src/` and `tests/` facts below are from before the cleanup; the block at the end is from after it, on `chore/comment-cleanup`. **Re-verify at slice entry**: the cleanup branch shifts every line number in `src/` and `tests/`, and its merge is what makes the after-block the truth on `main`. The adversarial pass (2026-10-14, a fresh agent) corrected the lines marked below.

- `assets/contract-checkpoint.md`: the checkpoint worker reads `# Autonomy`, `# Gate transcript`, `# Covered slices`, `# QA rows`, `# Log`; fixes a failing row in the files it names, verifies with the inner-loop command and names it under **Fixed**; the driver commits the lane.
- `src/checkpoint.rs:125-147`: the gate loop, until green or three reds; `src/pack.rs:398` builds the checkpoint pack, sections at `:472` (`# Gate transcript`), `:477` (`# Covered slices`), `:479` (`# QA rows`).
- `src/docs/doctor.rs:59` `run` prints Framework, gate, skills and drift; its one caller is `src/cli/docs.rs:308`. `/prd`, `/slice` and `/lanes` § 0 run it (`skills/prd/SKILL.md:20`, `skills/slice/SKILL.md:20`, `skills/lanes/SKILL.md:17`). `factory run`'s preflight is `src/docs/preflight.rs:98`, called from `src/docs/mod.rs:91`; it builds its own verdict from `docs check` and the skills verdict and never calls doctor. *Corrected 2026-10-14: an earlier line said `factory run` reads doctor.*
- `assets/retro.md:33-35`: `## Framework friction → skills repo`. `src/closeout.rs:356-373` `file_friction` files that section as an idea in the `[source]` checkout when it differs from the template.
- `skills/prune/SKILL.md:12`: "Application code is off-limits"; prune runs in polish `pr` mode (`skills/polish/SKILL.md:126`).
- `skills/`, `assets/`: `grep -i comment` hits only `assets/lookup.md:4` (`ctx refs` drops comments). No comment rule exists.
- `src/pack.rs:31` includes `assets/autonomy.md`; `src/pack.rs:236,293,377,484,569,620` push it into all six packs.
- `src/worker.rs:307`: `--append-system-prompt-file`. The pack is added to Claude Code's prompt and does not replace it, so the default "no comments, WHY only" rule reaches workers.
- `src/closeout.rs:2176-2179`: cites AD-1 and AD-30001 in a code comment.
- `src/checkpoint.rs:614`: "(FR-9)" means PRD-001's quota FR.
- `src/scheduler.rs:399-401`: the comment reads as the inverse of `checkpoint_waits = board.checkpoint_due && !free.is_empty()`.
- src+tests (Rust): 4,910 comment lines, 50,062 code lines, 692 comment lines citing an id (FR-3's first-row pattern, any case), in 100 files. `feat/isolated-lanes` (b2fca3c), `main` at 30123ca and at c913d36 hold the same `src/` and `tests/`. *Corrected 2026-10-14: an earlier line gave 4,508 and 634 for `main`; that was the pre-merge main (8458b30). FR-5's first draft counted 644 with a narrower pattern; 692 is the row's count.*
- PRD-005's own added lines (`git diff 8458b30 611dcd1 -- src tests`, `+` lines): 5,976 non-blank, 720 comment lines (12.0%), 106 of them citing an id (14.7%; 100 with the pattern case-sensitive). FR-1's baseline. `611dcd1` and the branch head `b2fca3c` hold the same `src/` and `tests/`.
- `src/retro.rs:162` `numbers()` builds the retro table between `factory:numbers` markers.
- `skills/slice/SKILL.md:83` (the trap line) and `:86` (every task traces to the PRD); `assets/contract-slice.md:17` (AD-n in the log), `:18` (the trap note).
- `skills/polish/SKILL.md`: § 1 Collect at `:29`, § 2 Verify at `:60`, § 3 Triage at `:78`, § 5 Fix at `:101`.
- `docs/features/004-skills/slice-01-spine.md:79`: "Write it in the doc comment."
- `src/db.rs:257-258`: "(slice 13)… before slice 13", a history note citing a slice.
- The five plan-named tests: `src/db.rs:1226`, `src/cli/report.rs:1155`, `src/quota.rs:427`, `src/cockpit.rs:356`, `tests/stubs.rs:273`.
- `tests/fixture/prd-004/` is read at `tests/docs_log.rs:298,301`.
- `src/docs/migrate.rs:1699`: `"{d}/prd.md (FR-13 frontmatter, prose header dropped)"`.
- `tests/stubs/claude:177` "(PRD-001 §17)"; `:525` "FR-10".
- `tests/stubs/cursor-review.jsonl`: a recorded cursor-agent stream, read by `src/review.rs:446` and `tests/review.rs:16`; it cites PRD-001, PRD-002 and five FRs.
- `Cargo.toml` has no `[lints]` table and there is no `clippy.toml`: `undocumented_unsafe_blocks` is off. Four `unsafe` blocks in `src/`: `src/review.rs:202,219,265`, `src/daemon.rs:50`.
- Non-comment Rust lines with an id-shaped string: about 400, `tests/lanes.rs` 81, `tests/closeout.rs` 68, `src/docs/migrate.rs` 36. Most are factory's own BLOCK-n, AD-n and CP-n data.
- `skills/prd/SKILL.md:134` (the proof row "names the test or command"); `skills/harness/references/skill-template.md:12` ("prove FR-n") and `:72` ("Name files by feature id").
- The four anecdotes: `skills/prd/SKILL.md:66` "PRD-001 FR-1", `skills/gate/SKILL.md:132` "CP-2 of PRD-001", `:134` "PRD-003", `skills/harness/SKILL.md:33` "PRD-001's two A/B runs". `skills/gate/SKILL.md:63` already names its repo.
- The field audits were read-only: comments on 2026-10-13 (cody-agent at d858bc4, arca-api at 91b4c59), names on 2026-10-14. Their file:line facts are theirs to re-verify. The checkouts: `~/Projects/CODESM/cody-agent` and `~/Projects/Prokey/arca-api`; a second `arca-api` under `~/Projects/ICG/` is not the audited one.

**After the cleanup** (`chore/comment-cleanup` at db7ae02: cecfc3f, a264b99, db7ae02; 2026-10-14; gate green, 570 tests):

- src+tests (Rust): 54,565 lines, 1,057 comment lines, 8 citing an id (FR-3's first-row pattern, any case), 571 `///` lines, 66 `//!` lines. Before, on `main` c913d36 with the same commands: 58,426, 4,910, 692, 3,039, 764. Diff: 108 files, 842 insertions, 4,769 deletions.
- The renamed tests: `src/db.rs:1129` `migrate_body_runs_on_a_version_3_store`, `src/quota.rs:377` `weights_count_cache_reads_at_a_tenth_and_output_at_five`, `src/cockpit.rs:324` `tokens_print_in_tenths_of_a_million_or_a_thousand`, `src/cli/report.rs:1067` `threshold_lines_compare_against_the_ctx_reversal_limits`, `tests/stubs.rs:262` `stub_claude_refuses_an_unknown_outcome`, `src/ctx/refs.rs:172` `test_dirs_match_by_segment_and_test_files_by_name`.
- `tests/fixture/log-without-covers/log.jsonl`, read at `tests/docs_log.rs:291,294`.
- `src/docs/migrate.rs:1632`: `"{d}/prd.md (frontmatter rewritten, prose header dropped)"`.
- `src/closeout.rs:1507,1531,1577,1662` and `src/scheduler.rs:2011`: the block texts no longer cite `autonomy.md`. Its three sibling texts (`src/checkpoint.rs:215`, `src/closeout.rs:173,1796`) never did.
- `tests/stubs/claude:154` "is not one the stub knows" (asserted at `tests/stubs.rs:270`); `:476` names the scenario step; `tests/stubs/gh:132` `displayTitle: 'Fixture'`.
- The eight id-citing comment lines left: `tests/quota.rs:377`, `tests/merge.rs:96`, `tests/closeout.rs:937`, `tests/checkpoints.rs:481,658,743,1368,1422`. All name the fixture's own slices. Rewritten in db7ae02: `src/cockpit.rs:589`, `src/commit.rs:148` (now `feat(NNN): slice-NN Title`), `tests/lanes.rs:771,968`, `tests/loop.rs:1096`. `src/cli/docs.rs:128` shows `PRD-NNN polish` (a264b99). *Corrected 2026-10-14: the first list was counted with a narrower pattern.*
- FR-3's other `fix` rows on the branch, as tuned: no hit. Before tuning they hit `src/measure.rs:282` and `tests/harness/mod.rs:83` (a date that explains a fixture), `src/skills.rs:20` (`SKILL.md` as a file name), `src/closeout.rs:132` (the `── polish ──` banner) and `src/scheduler.rs:2015` ("no longer" in a block text). The name row's known hits to settle at tuning: `ad1`..`ad3` locals in the test modules of `src/docs/log.rs` and `src/docs/check.rs`, and `SLICE_01`, `SLICE_02` in `tests/merge.rs`. Paths with an id outside `docs/`: only `tests/fixture/v2/docs/features/001-fixture/`.
- `src/closeout.rs:2176` and `src/scheduler.rs:399-401`: the wrong comments are gone, not patched.
- `// SAFETY:` lines, one line each, on every `unsafe` block: `src/review.rs:175,190,233`, `src/daemon.rs:39`; also `tests/review.rs:357,393,402`. *Corrected 2026-10-14: cecfc3f had dropped `src/review.rs:233`; db7ae02 restores it.*
- The multi-line stub headers kept: `tests/stubs/claude:3-62`, `tests/stubs/gh:3-25`, `tests/stubs/cursor-agent:3-23`, `tests/fixture/gate.sh:2-10`.
- The agents' eleven reports, with every unsure keep and every pattern, are in the session's scratch file, not in the repo; the patterns are the five new rows of FR-3 and the review questions in its polish step.
