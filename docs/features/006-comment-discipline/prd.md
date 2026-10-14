---
number: 006
slug: comment-discipline
title: "Comment and naming discipline"
# type: feature | bugfix | enhancement | refactor
type: feature
created: 2026-10-14
author: Emiliano Tisato
summary: |
  The plan stays in docs/. Code reads as if the plan never existed: no PRD, FR, AD, slice or checkpoint id in comments, names, paths, fixtures or strings, and a comment only for what the code cannot show. Four layers, because a prompt alone does not hold: one rule in every worker's pack, templates that stop modeling ids, a check in polish over added lines, and a one-time cleanup, because agents copy what is nearby. The retro tracks comment share and id hits. **Draft 2026-10-14, no interview yet:** OPEN-1, 3, 5 and 7 were settled in the research session; OPEN-2, 4 and 6 hold it. Research is in `research.md`.
---

# PRD-006 — Comment and naming discipline

Drafted from one session's research (2026-10-13/14): a 9-agent web and repo sweep with a skeptic per sweep (54 claims, 52 held), a comment audit of two field repos, and a second sweep for plan ids in names (§ 11 of the research). The full write-up is in [research.md](research.md). **The interview has not run.** The owner settled four OPENs in the session (2026-10-14); every other decision below is a proposal until its OPEN is settled.

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
- **A new CLI verb, `factory leaks`.** It is a new surface and breaks no existing contract.
- **One output line of `factory docs migrate` changes** (`src/docs/migrate.rs:1699`): it stops printing "FR-13".
- **The retro numbers table** (`src/retro.rs:162`) gains two columns. Old retros keep their table.
- **Existing code is rewritten only in factory, by the cleanup (FR-5).** Elsewhere it runs only where the owner points it.
- **No external contracts.**

## 3. FR-1 — One rule every worker reads: the plan stays in docs/

**Today.** No factory asset or skill says anything about comments (`grep -i comment skills/ assets/`) or about names. Workers get only the default prompt and, on this machine, the output style. The owner's rule against plan-named tests lives in one log line (`docs/features/002-docs-port/log.jsonl:82`), which no worker reads; PRD-005 broke it the same day. The `assets/autonomy.md` sections are: Who decides, The test, Mapping, What an unattended run may write, Never to get green, Tokens, Caps, Blocker protocol, The owner answers mid-run, Resuming.

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
- Do not copy what is nearby when it breaks this. Fix it when you touch it.
```

- **One principle, not a list of bans.** "Reads as if the plan never existed" covers cases a list would miss. The full id list lives in FR-3's check, not in the prompt.
- **It bans specific plan items, not the words.** In factory, PRD, slice and checkpoint are the product's own concepts: `struct Slice`, `parse_prd` and `Framework::V3` stay.
- **Own-code invariants keep their one line** (OPEN-1, settled). This is looser than `/no-comments`, which deletes them.
- **A public-contract line may join it** if OPEN-4 lands that way.

**Why a prompt alone does not hold.** Workers already get two no-comment rules, and 14% of factory's comment lines still cite an id. The rule is still needed: the `prd-004` fixture came back partly because no worker could read the owner's rule. FR-2 to FR-5 back it with inputs that stop modeling ids, a check and a cleanup.

**Where it lives.** The research synthesis put a comment rule in `assets/contract-slice.md`, next to the trap line (`:18`). This draft puts it in `autonomy.md` instead, for the reason under "Alternative that lost".

**Accepted cost.** About 200 tokens on every call of every worker. We take it because one wrong comment costs more: about 24% accuracy for the next reader.

**Alternative that lost.** Putting it in `contract-slice.md` only. Fix, merge and polish workers write code too, and the comment at `src/closeout.rs:2176` came from a polish run.

**Acceptance.** Every role's pack holds the section; a unit test over `pack.rs` checks it. A live run comes in below PRD-005's numbers, 11.9% of added lines as comments and 14% of comment lines citing an id, and adds no name with a plan id (PRD-005 added one). FR-4 measures it.

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

**Acceptance.** `grep -rniE "in a comment|code comment" skills/ assets/` finds only the ban. The four anecdote lines and the two harness-template lines read back without a factory id. A small PRD sliced after the change has no slice file asking for a comment or naming a test after an FR (read back).

## 5. FR-3 — Polish checks every added comment and name

**Today.** `/polish` has no comment or naming step (`skills/polish/SKILL.md`), and nothing measures either.

**Proposed (2026-10-14).**

- **A verb, `factory leaks [--base <ref>]`.** It reads `git diff -U0 <base>...HEAD` outside `docs/`: the added lines and the added file paths. An extension table tells comments from code: rs, go, php, js/ts/vue, py, sh, toml/yaml, json/jsonl. Markdown outside `docs/` is skipped, like READMEs. It reports:
  - **Ids in comments:** `(PRD|FR|AD|CP|OPEN|OQ|DEC|BLOCK|ASK)-\d+`, `slice[- ]?\d+`, `§`.
  - **Ids in names and paths:** the same items in identifiers, test titles and file or folder names, any case, joined by `-`, `_` or nothing (`prd_003`, `tests/fixture/prd-004`, `frNNN-` screenshots, `slice2Allowlists`, `section_14`).
  - **Ids in string literals,** outside the allowlist below (`'FR-157: Support switches…'`, `cp1-fr43-…`).
  - **History words in comments:** "until slice", "for now", "previously", "no longer", "added in".
  - **New comment blocks of 3 or more lines.**
  - **The comment share** of the added lines.

  It exits 1 on any id hit.
- **An allowlist in `project.toml`, because factory uses ids as data.** Factory numbers BLOCK-n, AD-n and CP-n itself, so about 400 non-comment Rust lines hold id-shaped strings today (`tests/lanes.rs` alone has 81). A path allowlist turns off the string check there. Names, paths and comments are always checked. Recordings of real output, like `tests/stubs/cursor-review.jsonl`, go on the list too.
- **Polish runs it beside its reviewers in § 1 Collect** (`skills/polish/SKILL.md:29`). Its hits pass § 2 Verify like any reviewer's and land in § 3's Fix bucket. Polish fixes them itself, with no question to the owner.
- **Polish's code review gets one question per surviving added comment,** starting with any security claim: does it still match the code beside it, and would a reader miss it? A grep cannot catch the `scheduler.rs:399` class, or cody's false security claim; a reader can.
- **It gets one more question for added names:** does any name point at the plan instead of the behavior? No regex catches a plan reference without a number, like `weights_match_the_prd_formula`, `…_a_later_slice_owns` or cody's `seedPre025Workspaces`.

**Accepted cost.** A new verb, a comment-syntax table per language, and a tuned name pattern. Short forms collide with real names: Go's `slice1`, the `cp1252` codepage. So the pattern is tuned against all three repos before it ships.

**Alternative that lost.** A grep written into the skill text. It cannot be unit-tested, and each repo would drift its own copy.

**Acceptance.**

- On a fixture branch with seeded lines, `factory leaks` lists the planted leaks and exits 1. The planted leaks are an id comment, an "until slice 4", a test named after an FR, and a fixture folder named `prd-9`. It leaves the real why, the foreign quirk and an allowlisted `BLOCK-1` alone, and prints the share.
- A read-only run over the whole of cody-agent and arca-api (the empty tree as base), and over factory after FR-5, reports no false hit: a reader agrees each one is a real leak.

## 6. FR-4 — The retro tracks comment share and id hits

**Today.** The retro's numbers table (`src/retro.rs:162`, between `<!-- factory:numbers -->` markers) has tokens, slices and tokens per slice, but no comment figures.

**Proposed (2026-10-14).** Add two columns, filled from `factory leaks` at ship time: the added comment share and the id hits in comments, names and strings. They are compared with the three earlier runs, as the table already does. This is how the owner sees whether FR-1 and FR-2 work, run over run.

**Accepted cost.** One more measurement at ship and a wider table.

**Acceptance.** A stub run to ship writes a `retro.md` whose numbers table holds both columns.

## 7. FR-5 — A cleanup skill for existing code

**Today.** The weeds are already planted: 692 id-citing comment lines in factory, 552 bare ids in arca's Go code, the cody tags above, and the names in research § 11. Because agents copy what is nearby, a rule that only applies going forward keeps losing to them. Renames alone do not hold either: cody's 22 renames (aaf6706) came back within two weeks, and factory's 0d55577 the same day, both with no rule a worker reads.

**Proposed (2026-10-14).** Add a `/leaks` skill, a weedkiller, shaped like `/prune` with a `full` mode (whole repo) and a `pr` mode (the branch). It starts from `factory leaks` hits and reads on from there. Per comment or name:

- **Delete:** restating comments, docblocks that repeat the signature, and id and history comments.
- **Rewrite:** a comment whose fact is real but whose label is an id becomes the condition itself.
- **Rename:** a name, path or string that cites a plan item gets the behavior's name, and every reference follows.
- **Check:** each kept why or invariant against the code. A wrong one is a finding, not a deletion.
- **Propose:** a test where an invariant can be pinned.
- **Park:** a surprise in our own code that needs a redesign becomes an idea (`docs idea add`). The cleanup never redesigns code.

Verify after each batch with the gate, as `/prune` does.

**Factory's rename set** (research § 11), with the better names the sweep proposed:

- `src/db.rs:1226` `the_prd_003_migrate_body_runs_on_a_version_3_store` → `a_schema_v2_migration_is_a_no_op_on_a_version_3_store`.
- `src/cli/report.rs:1155` `the_threshold_lines_compare_with_the_section_14_constants` → `threshold_lines_compare_against_the_ctx_reversal_limits`.
- `src/quota.rs:427` `weights_match_the_prd_formula` → `weights_count_cache_reads_at_a_tenth_and_output_at_five`.
- `src/cockpit.rs:356` `tokens_read_like_the_prd` → a name that says what the format is.
- `tests/stubs.rs:273` `stub_claude_refuses_an_outcome_a_later_slice_owns` → `stub_claude_refuses_an_unknown_outcome`.
- `tests/fixture/prd-004/log.jsonl` → `tests/fixture/log-before-covers.jsonl`, with its two references (`tests/docs_log.rs:298,301`).
- `src/docs/migrate.rs:1699` prints "FR-13 frontmatter" into every repo it migrates; it names the seven keys instead.
- `tests/stubs/claude:177` "(PRD-001 §17)" and `:525` "FR-10" drop the citation, with the one assertion that matches the first.

**Never renamed:** migration files, because the database records their names. In factory, recordings of real output stay as recorded and sit on FR-3's allowlist.

**Accepted cost.** A large one-time diff per repo. It is reviewed as its own feature, never folded into a build.

**Acceptance.** Run on factory itself: id-citing comment lines go from 692 to 0, `factory leaks` over the whole repo finds nothing, and the gate stays green. The owner's sample read (§ QA plan) finds nothing lost that a reader would miss.

## 8. Decisions log

- ~~**OPEN-1** — Invariants in our own code: keep them as a one-line comment, or require a test, type or rename and delete the comment (the `/no-comments` line)?~~ **Settled 2026-10-14:** keep, one line, in FR-1's rule. Strict would lose the class both audits say a reader would miss (races, coupled timeouts), because concurrency rarely renames away.
- **OPEN-2** — Does the polish check block or only report?
  - Ids are mechanical, so they are Fix and polish clears them itself.
  - "Is it still true" and "does this name point at the plan" need a reader, so they stay review questions and only report.
  - **Recommended:** exactly that split. No new owner question.
- ~~**OPEN-3** — Is FR-5's skill in this PRD, or an idea for later?~~ **Settled 2026-10-14:** in, as the fourth layer: the untouched ids keep seeding copies. FR-1's "Fix it when you touch it" also cleans touched lines from then on. cody and arca run the skill later as their own features (OPEN-6).
- **OPEN-4** — Which doc comments count as a "public contract"? Are docblocks on private functions banned, or allowed when they state a contract?
  - Factory is a binary crate, so its `pub` items are not public API.
  - Laravel PHPDoc that PHPStan needs is types, not prose.
  - arca's new private functions carry a doc comment 64% of the time, and 17 of its 60 sampled comments repeat the signature.
  - FR-1's accepted text has no public-contract line yet, so a "keep" answer adds one.
  - **Recommended:** a library crate's pub API, an HTTP or CLI contract, and type annotations a checker reads. Every other docblock, private ones included, follows the rule for ordinary comments.
- ~~**OPEN-5** — Ban `// FR-NN:` test headers and FR ids in test names?~~ **Settled 2026-10-14:** banned, in FR-1's rule: "The test never names the FR". The QA plan is the only map from FR to test (research § 11, option C), so `--filter=FR-157` stops working.
- **OPEN-6** — Should the cleanup also run on cody-agent and arca-api now?
  - **Recommended:** no, not in this PRD; each runs as its own feature once the skill ships.
  - The wrong security comment in cody (`DynamicSetResolver.php:140`) is already on cody's board (`docs/ideas.md`, parked 2026-09-06). It needs a fix there, not here.
- ~~**OPEN-7** — Does this PRD cover plan ids in names too (tests, fixtures, classes, strings), or only comments?~~ **Settled 2026-10-14:** same PRD. Same root cause, one rule (FR-1), one set of template edits (FR-2), one check (FR-3) and one cleanup (FR-5). The research's options A to F are in; G (rename on touch only) is the rule's last line, on top of the cleanup. E (`/prune`) is not needed.

## 9. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` *(declared in project.toml)*
**Surface**: the CLI (`factory leaks`), the worker packs, the skill texts.
**Harness**: the CLI test harness from PRD-005, `tests/harness/mod.rs`, with the stub worker `tests/stubs/claude`.
**Seed state**: a fixture branch with seeded lines: an id comment, a history note, a restating line, a real why, a real foreign quirk, a test named after an FR, a fixture folder named `prd-9`, and an allowlisted `BLOCK-1` string.

| FR | Proof on the real surface | Evidence | Human? |
|---|---|---|---|
| FR-1 | run the pack test: every role's pack holds `## The plan stays in docs/`. Then a live factory run on a small PRD: its retro numbers show comment share and id hits below PRD-005's | test transcript; `retro.md` numbers table | no |
| FR-2 | run `grep -rniE "in a comment\|code comment" skills/ assets/`: only the ban lines. Read back the four anecdote lines and the two harness-template lines. `/slice` a small PRD: no slice file asks for a comment or names a test after an FR | grep transcript; lines and slice files read back | no |
| FR-3 | run `factory leaks --base main` on the seeded fixture branch: it lists the id comment, the history note, the FR test name and the `prd-9` folder, skips the why, the quirk and the allowlisted string, prints the share, exits 1. Then run it read-only over cody-agent and arca-api with the empty tree as base, and read the hits for false ones | CLI transcripts; the false-hit read | no |
| FR-4 | run a stub run to ship: `retro.md` holds the two new columns | `retro.md` read back | no |
| FR-5 | run `/leaks full` on factory: id-citing comment lines 692 → 0, the eight renames done, `factory leaks` over the whole repo finds nothing, gate green | before/after counts; CLI and gate transcripts | no |
| FR-5 | the owner reads 20 comments the cleanup kept, 20 it deleted and the renames, and judges whether a reader would miss anything | owner's note in `retro.md` | yes |

## 10. Slices

_(Not sliced yet. Run `/slice`, or write "No slices — build straight from this document" if it is small.)_

## 11. Non-goals

- **Cleaning cody-agent and arca-api.** Each is its own feature in its own repo, run with FR-5's skill once it ships (OPEN-6). That includes their harness skills' feature maps, keyed by FR today (cody's `verify-cody-agent` carries about 130 plan mentions).
- **Prose in `docs/`, READMEs, commit messages and PR bodies.** Narration and ids belong there.
- **Factory's domain words.** PRD, slice, checkpoint, `Framework::V3` and `LEGACY_DOC_DIRS` name the product's concepts, not a plan item.
- **Renaming migrations.** The database records their names.
- **Not handled: per-language comment linters** (clippy `missing_docs`, golangci `godot`, PHP-CS-Fixer comment rules). Accepted 2026-10-14: one tested verb covers every stack the owner uses. Revisit if the retro shows id hits creeping back past the check.

## 12. Verification notes

Checked against `feat/isolated-lanes` (b2fca3c) 2026-10-14; the naming lines against `main` (30123ca) the same day. **Re-verify at slice entry.**

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
- The five plan-named tests: `src/db.rs:1226`, `src/cli/report.rs:1155`, `src/quota.rs:427`, `src/cockpit.rs:356`, `tests/stubs.rs:273`.
- `tests/fixture/prd-004/` is read at `tests/docs_log.rs:298,301`.
- `src/docs/migrate.rs:1699`: `"{d}/prd.md (FR-13 frontmatter, prose header dropped)"`.
- `tests/stubs/claude:177` "(PRD-001 §17)"; `:525` "FR-10".
- `tests/stubs/cursor-review.jsonl`: a recorded cursor-agent stream, read by `src/review.rs:446` and `tests/review.rs:16`; it cites PRD-001, PRD-002 and five FRs.
- Non-comment Rust lines with an id-shaped string: about 400, `tests/lanes.rs` 81, `tests/closeout.rs` 68, `src/docs/migrate.rs` 36. Most are factory's own BLOCK-n, AD-n and CP-n data.
- `skills/prd/SKILL.md:134` (the proof row "names the test or command"); `skills/harness/references/skill-template.md:12` ("prove FR-n") and `:72` ("Name files by feature id").
- The four anecdotes: `skills/prd/SKILL.md:66` "PRD-001 FR-1", `skills/gate/SKILL.md:132` "CP-2 of PRD-001", `:134` "PRD-003", `skills/harness/SKILL.md:33` "PRD-001's two A/B runs". `skills/gate/SKILL.md:63` already names its repo.
- The field audits were read-only: comments on 2026-10-13 (cody-agent at d858bc4, arca-api at 91b4c59), names on 2026-10-14. Their file:line facts are theirs to re-verify.
