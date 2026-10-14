# PRD-006 research — Code comments and agents

Collected 2026-10-13/14 for [prd.md](prd.md). Three sources:

1. **A research workflow** (`code-comments-for-agents`, run `wf_3d555c22-f8a`). Four sweeps (studies, vendor guidance, mechanics, this repo), a skeptic per sweep, one synthesis. 54 claims checked: **52 held, 2 refuted** (§ 7).
2. **lauren's tweet** and pstack's open-source `/no-comments` skill (§ 6).
3. **Comment audits of two field repos**, cody-agent and arca-api (§ 8, § 9). Both were built 100% with this framework, V1 and V2 skills, then the v3 CLI. So every era in those audits is ours, including the "before" baseline.

## 1. Bottom line

A wrong comment misleads an agent badly, while a missing one costs little, so default to very few. Keep the one-line why, the invariant, the gotcha, and the contract on a public item. Ban restating comments and ticket references. Check that mechanically, because prose rules already fail here.

## 2. Comments as input

- **Accurate comments barely help frontier models.** Correct docstrings did not raise GPT-4's test pass rate ([Macke & Doyle, NAACL Findings 2024](https://arxiv.org/abs/2404.03114)). The only agentic ablation found that stripping comments slightly helped GPT-5-mini on SWE-bench Verified. GPT-5.2 trended the other way, not significantly ([Antimemetics Institute, blog](https://antimemetics.institute/comment-ablation)). Gains were larger for weaker models on obscure code ([MITRE](https://arxiv.org/abs/2506.11007)).
- **Wrong comments hurt, and models trust them.** Misleading comments cut accuracy about 24% on average across 17 models. Telling the model to ignore comments barely helped ([CodeCrash, NeurIPS 2025](https://arxiv.org/abs/2504.14119)). They hurt fault localisation more than misleading names ([Haroon et al., ICST 2026](https://arxiv.org/abs/2504.04372)). Outdated comments pull completion toward the old code ([Xia et al., ASE 2026](https://doi.org/10.1145/3832783.3834359), abstract only).
- **Stated results are the worst kind.** A wrong "returns X" comment cost o3-mini-high, a reasoning model, up to 28%.
- **Review is sturdier, until comments are tuned.** Hand-written deceptive comments barely fooled 2026 frontier reviewers ([Thornton](https://arxiv.org/abs/2602.16741)). Comments an agent tuned against research detectors evaded them over 90% of the time ([ALIBI](https://arxiv.org/abs/2607.24964)). Factory has that shape: builders write, reviewers read.
- **Placement matters.** In the owner's 1,731 sessions, agents read code in roughly 60-line windows, so a line comment is usually in view. Commit messages are not: `git blame` ran in 0.6% of sessions.

Caveat: most studies are single-shot, with 2024-25 models.

## 3. Comments as output

- **Vendors converged on "default to no comments".** Claude Code 2.1.296 allows only a non-obvious WHY. It bans task and issue references because they "rot" ([prompt mirror](https://github.com/Piebald-AI/claude-code-system-prompts/blob/main/system-prompts/system-prompt-comment-why-only-guidance.md)). Codex says comments "should be rare" ([gpt_5_codex_prompt.md](https://github.com/openai/codex/blob/main/codex-rs/core/gpt_5_codex_prompt.md)).
- **Prose rules slip.** Users report CLAUDE.md rules ignored within 1-3 turns ([claude-code#65961](https://github.com/anthropics/claude-code/issues/65961)). Others report success with rules plus a review pass.
- **Agent-written comments are often false.** GPT-4's Java comments held a false statement about 1 time in 5 ([Kang et al.](https://arxiv.org/abs/2406.14836)). In human repos, changes that left comments inconsistent preceded bugs 1.5x more often ([Radmanesh et al.](https://arxiv.org/abs/2409.10781)).

## 4. Factory's own numbers

- **Workers already get two no-comment rules, and they do not hold.** Claude Code's default prompt reaches workers, because `src/worker.rs:307` appends the pack. The owner's output style adds "Fewer comments beat more"; all 16 PRD-005 worker streams show it. Factory adds no rule of its own.
- **feat/isolated-lanes** adds 691 comment lines to 5,099 code lines: 11.9% of lines, 23.4% of characters.
- **About 12-14% of Rust comment lines cite PRD, FR, AD or slice ids.** 224 lines cite a bare FR-n.
- **A 63-block sample** (one unblinded rater): 38% why or invariant, 24% public docs, 13% restating, 13% thin summaries, 10% tickets.
- **Real harm:**
  - `src/closeout.rs:2176-2179` cites AD-1 and AD-30001. Both resolve to the wrong decisions.
  - `src/checkpoint.rs:614` "(FR-9)" means PRD-001's quota FR, while PRD-005's FR-9 is `/lanes`.
  - `src/scheduler.rs:399-401` reads as the inverse of the flag below it.

## 5. Which kinds earn their place

**Keep:**

- Invariants and external-tool traps (`src/land.rs:61`, `src/scheduler.rs:1567`).
- Why code is ordered or shaped a certain way (`src/closeout.rs:332`).
- Public-item docs that state a contract and its edge cases (`src/docs/project.rs:86`).

**Cut:**

- Restating comments (`src/lanes.rs:274`, `src/scheduler.rs:1729`). Rename instead.
- Ticket and history references. Ids restart per PRD and already resolve wrong here. Keep the fact, and replace the label with the condition.
- Change narration and "this is correct because" notes. That is the builder talking to the reviewer.

**TODOs:** unmeasured. Factory has none and parks work with `docs idea add`.

**Uncertain:**

- No study separates why comments from what comments, or measures ticket references or API docs for agents. The keep/cut split is a judgment, inferred from vendor guidance and local harm.
- The agentic evidence is one blog, two models, disagreeing task by task.
- Why the default rule fails here is not proven. Candidates: ticket-heavy packs, density matching, spec wording. The field audits (§ 8, § 9) point at all three.

## 6. lauren's tweet and pstack's /no-comments

**The tweet.** lauren ([@poteto](https://x.com/poteto/status/2110149626133438732)), 2026-10-13, about 2.6k likes:

> ban and delete most if not all code comments! agents love copying what already exists. a seemingly harmless comment explaining some temporary workaround is one of the worst viruses… like a weed it'll spread… use pstack's /no-comments skill as a weedkiller…

**The skill is open source**, in `cursor/plugins`:

- **Skill:** [pstack/skills/no-comments/SKILL.md](https://github.com/cursor/plugins/blob/main/pstack/skills/no-comments/SKILL.md). It spawns the agent below on a diff (default: against `main`). It reviews the agent's report, fixes accepted findings, and runs `/architect` once when a fix needs a shape. For a constraint comment ("do not remove"), it offers the cheapest type, runtime check, test or CI lint, then deletes the comment.
- **Agent:** [pstack/agents/comment-sicko.md](https://github.com/cursor/plugins/blob/main/pstack/agents/comment-sicko.md). It deletes comments and never touches application code.

**Its keep-list**, the only comments that survive:

- Legal or license headers.
- Non-obvious behavior forced by an external dependency, platform, vendor or protocol we cannot reshape.
- `// prettier-ignore`, and lint suppressions whose rule is faulty, pedantic or style-only.
- Doc comments that define a public API contract.
- Issue or RFC links that explain a constraint code cannot express.

**Its rules of thumb:**

- "Surprises in our own code are meat." They die, and the symbol is marked `MUST KILL` for a rename, extract, type or redesign that makes the behavior obvious.
- "When I am not sure a keep clause applies, the comment dies."
- `IMPORTANT`, `do not remove`, `fine for now` and long justifications are "scent, not conviction". It checks the claim against nearby code first.
- A suppression whose rule protects correctness or safety is itself a `MUST KILL`.

**Verdict for factory.** Take the shape for FR-5: a cleanup agent, a review of its report, and a constraint encoded as a test before its comment goes. The keep-list is close to the right line. Change three things:

1. **Keep invariants in our own code that no rename expresses** (races, coupled timeouts, cross-module couplings). Both field audits flag that class as the one a reader would miss (PRD OPEN-1).
2. **Allow links to external trackers only.** Internal ids rot.
3. **In an unattended run, park `MUST KILL` refactors as ideas** instead of redesigning.

**Where the tweet goes beyond the evidence** (§ 10): it is one practitioner's opinion. "Ban most if not all" deletes the class above. And what spreads in these repos is plan ids and test headers, not workaround notes.

## 7. Refuted claims

1. **"Workers get Claude Code's default prompt unmodified."** The append is right (`src/worker.rs:307-308`, `--append-system-prompt-file`; the `--settings` file holds only deny rules and a hook). "Unmodified" is wrong: `~/.claude/settings.json` sets the Attention-kind output style with `keep-coding-instructions: true`. Its "Code comments and docs" section reaches all 16 PRD-005 worker streams (`output_style: "Attention-kind"`, Claude Code 2.1.296). The default prompt in that binary says: "Default to writing no comments… Don't reference the current task, fix, or callers… since those belong in the PR description and rot." So something overrides both rules.
2. **"Factory's lookup tool shows a doc comment above a definition as context."** The noise half holds: `ctx refs` drops comment and string hits (`assets/lookup.md:4`), and `ctx def` drops candidates inside comments (`src/ctx/def.rs:139`). The context half rests on a pre-build sketch (`001-driver/ctx-research.md:113-118`). The shipped `ctx def` starts at the `fn` line and does not print the `///` lines above it. Inline comments inside bodies do get printed.

## 8. Field audit: cody-agent

`/home/emiliano/Projects/CODESM/cody-agent` at d858bc4. Laravel and Vue. Features 003-027, 2026-07-23 to 10-12. Read-only.

**Density** (comment lines as a share of added non-blank lines):

| Era | src | src prose only | tests |
|---|---|---|---|
| Earliest (06-13 to 07-18) | 8.6% | 4.1% | 1.7% |
| Framework, all features | 21.7% | 15.7% | 13.2% |

By feature group: 20.4% (005-014), 20.6% (015-020), 26.5% (021-025), then 027 at 55%. 4,803 comment blocks in all. **Docblocks are 68%** of framework-era comments.

**A 63-block sample:**

| Class | n |
|---|---|
| Why or intent | 22 |
| Invariant | 12 |
| Docblock that repeats the signature | 11 |
| Contract docblock | 6 |
| External quirk | 5 |
| Ticket | 4 |
| Restates the code | 3 |

18 of 63 carry an id tag. No TODOs.

**Stale or wrong:**

- `DynamicSetResolver.php:140`: **a wrong security claim.** It says the widget re-checks the link scheme, and it does not. Flagged 09-06 in `docs/ideas.md:96`, still unfixed.
- `AssetIngestService.php:31` "PDF ingest path lands in slice 02", while `ingestUploadPdf` exists at `:56`.
- `ContentFeedSyncCommand.php:10` and `AgentRunner.php:293`: "until slice N" for shipped slices.
- `AgentFaqController.php:20` names a class that no longer exists.
- A script comment promised 53 tools when there were 55, and it misled a worker (`docs/features/027-*/agent-log.md:17`).

**Spread:**

- Ticket tags: 0.7%, 5.5%, 7.2%, then 12.4% of comment lines. By feature, 4-6% of blocks (005, 007, 011) rose to 39% (021) and 51% (025).
- `AD-n` tags started in one file in 021 (`SecurityController.php:48`, 954cb1c) and reached 46 blocks by 025. 79 of 124 AD references name no feature, and AD-12 exists in both 021 and 025.
- A browser-test header, "FR-n on the surface X touches", spread from 1 file (019) to 5, 7, then 6.
- Stub docblocks from 005 were copied by 024.

**Rules in force:**

- `CLAUDE.md:137`: "Prefer PHPDoc blocks… Only add inline comments for exceptionally complex logic". A laravel-best-practices skill contradicts it.
- Slice briefs ask for comments: 25 of 148 slice files mention them. Examples: "State the invariant in a comment" (013/slice-01:56), "a code comment pointing at slice 03" (024/slice-01:45).

**Verdict.** **8%** of the sample survives `/no-comments` strictly, 30% if contract docblocks count. A reader would miss the invariants (`PromptCompiler.php:237`) and the "deliberately" notes (`routes/web.php:155`).

## 9. Field audit: arca-api

`/home/emiliano/Projects/Prokey/arca-api` at 91b4c59. Go. Features 012-024, 2026-09-16 to 10-12. Read-only.

**Density:** 4% (05-23 to 07-03), 19% (07-27 to 09-15), 21% under the v3 factory. Without feature 012, which hit 49%, the factory era is 15%. New **private** functions with a doc comment: 5%, then 39%, then a 64% median.

**A 60-comment sample:**

| Class | n |
|---|---|
| Doc that repeats the signature | 17 |
| Why | 14 |
| External quirk | 9 |
| Invariant | 8 |
| Contract | 8 |
| Restates the code | 3 |
| Ticket | 1, plus 5 tagged |

No TODOs.

**Stale or wrong:**

- `internal/caea/cap_policy.go:30` "Slice 04 makes these editable per register… Until then". It shipped in e947135.
- That stale comment became a template. Slice `012/slice-04-chain-backoff.md:271` told the builder to document constants "like `DefaultCaps` (cap_policy.go:26-42)". The builder copied the style into `internal/caea/informe_backoff.go:16`.

**Spread:**

- `// FR-NN:` test headers grew from 1 test to 45. They open 36% of 021's test comments.
- Markdown bold inside Go comments started in one CAEA file. Feature 012, working in that package, added 67. Features in other packages added almost none. Agents copy what is nearby.
- 552 bare ids in code. "OPEN-5" appears 11 times and means different questions in PRD-012 and PRD-013.
- Two runs of the same PRD (024) tagged 14% and 2% of their comments. Tagging is run noise, not a need of the PRD.

**Rules in force:** `AGENTS.md:86` says to extract a function instead of writing a comment. 012's PRD and slices mention comments 36 times, and 012 is the densest feature.

**Verdict.** **18%** survives `/no-comments` strictly, and 35% is pure noise. A reader would miss own-code invariants such as `internal/http/manage_apikey.go:159` and `internal/queue/jobs.go:381`.

## 10. The final synthesis

The workflow's synthesis was rerun with the tweet, the keep-list and both audits as input (task `w2zr1oxog`, 2026-10-14). It agrees with § 1-5. These points are new:

- **More evidence:**
  - Misleading comments cut output prediction by 21-29% across the 17 models in CodeCrash.
  - Reasoning models mostly ignore plainly wrong comments. A wrong *result* is the exception.
  - On Claude Code with Sonnet 4.6, TODO and lint-suppression markers had almost no effect on what the agent did ([SonarSource](https://arxiv.org/abs/2605.20049)).
  - A TODO that described a problem made Copilot reproduce the debt in 29 of 82 cases ([O'Brien et al., ICSE 2024](https://doi.org/10.1145/3597503.3639176)). This is the best support for lauren's "workaround virus".
- **How agents read here.** 83% of mid-file reads in the owner's transcripts included a comment line. Comments are 21.8% of the characters in factory's `src/`, and agents pay for them on every read.
- **Factory's own trend.** Density did not rise: 8.9% of lines on both `main` and this branch. The ids did carry over. 307 of 435 plan references name no PRD, and the wrong AD refs at `src/closeout.rs:2176` came from PRD-004.
- **Factory's own specs ask for comments:** `docs/features/004-skills/slice-01-spine.md:79` "Write it in the doc comment."
- **A history note to fix:** `src/db.rs:257-258` "(slice 13)… before slice 13" should name the real condition.
- **Keep "deliberately" notes**, which stop a well-meant fix. Examples: cody `routes/web.php:155`, factory `src/events.rs:539`. The latter also cites "PRD-005 FR-6", which should become the condition.
- **Ban test and section headers** like "FR-n on surface X". They spread fastest. Ban TODOs too; `docs idea add` already parks the work.
- **Where the rule lives**, as the synthesis proposed it:
  - Builder contract: `assets/contract-slice.md`, next to the trap line (`:18`).
  - `/slice` § 4 (`skills/slice/SKILL.md:74`): a task never asks for a comment. A fact worth keeping becomes a test or a log note.
  - `/polish`: a scripted check over added lines only, with hits triaged as Fix.
- **Review order:** the reviewer checks each remaining comment against its code, starting with security claims.
- **Its grep**, a starting point for FR-3's table:

  ```
  git diff -U0 "$BASE"... -- '*.rs' '*.go' '*.php' '*.ts' '*.vue' \
    | grep -E '^\+\s*(//|#|\*|/\*)' \
    | grep -inE '\b(slice[ -]?[0-9]+|PRD-[0-9]+|(FR|AD|OQ|OPEN|CP)-[0-9]+)\b'
  ```

**Still uncertain:**

- No study separates "why" comments from "what" comments, or measures plan-id comments for agents.
- Most input studies ask one question about one function, with 2024-25 models.
- The evidence that agents copy nearby comment density is weak (r 0.31-0.47, small samples), though all three repos agree.

**Its questions**, now PRD OPEN-2, OPEN-3 and OPEN-4:

1. Should the id check fail the gate, or only send findings to polish triage?
2. Clean the ~630 existing id lines now, or only the lines each diff touches?
3. Ban docblocks on private functions, or allow them when they state a contract?
