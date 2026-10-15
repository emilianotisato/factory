# PRD-006 research — Code comments and agents

Collected 2026-10-13/14 for [PRD-006](../archive/006-comment-discipline/prd.md). Three sources:

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

`~/Projects/CODESM/cody-agent` at d858bc4. Laravel and Vue. Features 003-027, 2026-07-23 to 10-12. Read-only.

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

`~/Projects/Prokey/arca-api` at 91b4c59. Go. Features 012-024, 2026-09-16 to 10-12. Read-only.

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
- **Factory's own specs ask for comments:** `docs/features/004-skills/slice-01-spine.md:79` (in git history) "Write it in the doc comment."
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

## 11. Plan ids in names, not just comments

The owner's follow-up (2026-10-14): ids leak into implementation code too, mainly test names and some classes. His example: `tests/fixture/prd-004`. "Names express things. Six months from now, what does prd-004 express?"

A second workflow (`tracking-ids-in-code`, run `wf_f102f8d3-b03`) swept cody-agent, arca-api and factory for plan ids in anything the program or test runner uses: names, paths, fixtures and runtime strings. Comments were out of scope. Two sweeps ran per field repo, plus one on factory, and a skeptic per repo re-ran the counts. Findings: cody 26 (8 refuted or corrected), arca 27 (4), factory 7 (2). Only verified claims are below. Nothing is decided yet.

**Bottom line.** Plan ids leak into names and runtime strings in all three repos, mostly in test code. cody and arca carry most of them; factory has a handful. No instruction asks for id names. But QA plans and Prove tasks key everything by FR, and no rule keeps ids in the docs.

**What leaks:**

| Pattern | cody | arca | factory |
|---|---|---|---|
| Test titles or functions | 96 of 3,154 (Browser: 61 of 196) | 56 of 130 drive titles; 20 of 2,552 Go funcs | 5 of 772 fns |
| Screenshot, evidence, run names | 105 `frNNN-`, 12 stage-named | 51 files; 131 of 184 run dirs | 1 fixture dir |
| Test output | 22 STDERR lines | 93 console lines; 55 Go strings | 2 stub strings |
| Fixture data | ~62 | ~32, some persisted to the DB | 1 frozen review, 18 ids |
| App code, migrations, seeders | 12 literals | 15 of 17 allowlist builders; 3 error messages | 1 CLI string |

**Examples, with a better name:**

- cody `tests/Browser/WorkspaceSwitcherTest.php:103` `'FR-157: Support switches…'`: drop the prefix. The sentence already says what it tests.
- cody `tests/Browser/LapsedReadOnlyBrowserTest.php:48` `'cs_test_cp3_canceled'` → `'cs_test_canceled'`.
- cody `tests/Feature/Billing/BillingMigrationTest.php:41` `seedPre025Workspaces` → `seedUnbilledWorkspaces`.
- arca `.claude/skills/verify-arca/drive/manage-health-pin.spec.ts:16` `'FR-61 pin'` → `'an operator pins a taxpayer to a health state and the pin survives a refresh'`.
- arca `internal/audit/writer.go:134` `slice2Allowlists` → `tenantAndAdminUserAllowlists`.
- arca drive fixtures write `cp1-fr43-…` credential names and refs into the platform DB (`devoluciones-live.spec.ts:143`).
- factory `tests/fixture/prd-004/log.jsonl` → `tests/fixture/log-before-covers.jsonl`.
- factory `src/db.rs:1226` `the_prd_003_migrate_body_runs_on_a_version_3_store` → `a_schema_v2_migration_is_a_no_op_on_a_version_3_store`.

**How it spreads:**

- **cody, in two waves.**
  - Vitest `(FR-n)` suffixes began at a0e40f9 (2026-07-20). aaf6706 (2026-08-04) stripped 22 of them, with the message "Names describe contracts, not the PR that wrote them". They came back at ac6c380 (2026-08-17).
  - The `FR-NNN:` title prefix began at 6209dcb (2026-09-16, PRD-021).
  - Titles with an FR id, by merge: 014 4, 019 13, 021 36, 023 49, 025 62.
  - Screenshots: 0 through 018, then 20, 76, and 105 by 025.
- **arca, in one harness era.** Drive specs arrived at 6f4f77b (2026-09-07). New specs with an id in the title: PRD-013 13 of 14, PRD-014 9 of 9, PRD-015 8 of 8, and every spec in PRD-018 to 020. All 19 Go FR names came from PRD-012, on one day.
- **factory, the smallest, and a relapse.**
  - PRD-002 added plan-named tests (`prd_001_copy_migrates`, `this_repos_prds_get_the_seven_keys`) and a frozen copy of this repo's docs (`tests/fixture/repo-at-a7e89bf/`).
  - The owner had them removed: 0d55577, 2026-10-13, 3,145 lines deleted. His review: "no test reads this repo's live or frozen docs" (`docs/archive/002-docs-port/log.jsonl:82`).
  - The rule went nowhere a worker reads. PRD-005 slice 05 recreated the pattern that same evening as `tests/fixture/prd-004` (f35ed4c, 20:42).

**Where it comes from.** No instruction asks for id names. Factory's shipped assets still model them:

- `assets/docs/prd.scaffold.md:41-43` keys the QA plan by FR.
- `assets/docs/slice.scaffold.md:19`: "**Source**: §_(n)_ FR-_(x)_".
- `skills/prd/SKILL.md:134`: the proof row "names the test or command", which pulls test names toward the row id.
- `skills/harness/references/skill-template.md:12` lists the trigger "prove FR-n". `:72` says "Name files by feature id". Its examples are behavior names (`rules-create.png`), but in factory "feature id" reads as a plan id.
- `assets/pr-body.md:30` puts `FR-{n}` and `CP-{n}` in a table.
- Four prose lines cite bare ids that mean something else in other repos: `skills/gate/SKILL.md:132`, `:134`, `skills/prd/SKILL.md:66`, `skills/harness/SKILL.md:33`.

The field repos model it directly:

- cody `.claude/skills/verify-cody-agent/SKILL.md:218` "RUN=fr159-mcp". That skill carries about 130 PRD/FR/AD mentions, and its feature map is keyed "PRD-021 FR-159".
- cody `docs/archive/021-support-team-access/slice-04-staff-identity.md:104` "- FR-150: a plain MaaS user…". Test titles copy this format.
- cody `CLAUDE.md:66` says to copy sibling naming, so one prefixed file spreads to the next.
- arca `docs/archive/012-caea-informe-scheduler/slice-01-series-ordered-read.md:153` "The FR-1 row of prd.md § QA plan". It became `TestProveFR1…`.

**Counter-rules exist, but no worker reads them.**
- cody's lives only in the aaf6706 commit message.
- `skills/prune/references/rules.md:21` flags test names that describe implementation steps, not plan ids.
- arca's `openapi/public_text_test.go:9-15` blocks plan ids in public docs only.

**Harm found:**

- **Renames paid for, then undone.**
  - cody aaf6706 renamed 22 titles and 8 files, and both patterns came back within two weeks.
  - In factory, 0d55577's cleanup was undone the same day.
- **Archive cost.** cody `TestBrowserCommandTest.php:325-346` exists only to stop a plan-doc path cited in app code from rotting. Archiving PRD-023 still had to edit `app/Console/Commands/TestBrowserCommand.php:31` (3539026).
- **Recorded confusion.** In factory, a removal grep in PRD-005 hit the prd-004 fixture (`docs/archive/005-isolated-lanes/log.jsonl:31`, AD-18).
- **Collisions.**
  - In cody, AD-10002 means four different decisions, and one is cited at `tests/Browser/BillingPolishBrowserTest.php:134`. `tests/Feature/Knowledge/FaqInboxKindTest.php:131` "(DEC-8)" matches the wrong entry.
  - In arca, `[slice 10]` means offline episodes (`episode.spec.ts:70`) and also device minting (`device-minting.spec.ts:35`). Slice numbers 03-08, 10, 11 and 13 all repeat across PRDs.
  - In factory, `tests/stubs/claude:525` "FR-10" means PRD-001's stub harness, while PRD-005's FR-10 is `lanes prune`. `src/docs/migrate.rs:1699` prints "FR-13" into repos that have their own FR-13.
- **Coupling and stale text.** cody `tests/Browser/BrowserTestMarkerTest.php:25` asserts a title that contains FR-170. arca `internal/tax/calc.go:286` tells API users "v1.5", while the spec is at 1.16.0.

**What is legitimate:**

- Factory's domain words: PRD, slice, checkpoint, `Framework::V3` (`src/docs/doctor.rs:19-24`), `LEGACY_DOC_DIRS` (`src/docs/check.rs:31`).
- External ids, such as ARCA error codes (`internal/padron/lookup_1502_test.go`). None of the three repos names anything after an upstream ticket.
- `Legacy*` names for old inputs that really exist, such as bookmarks.
- Ids in docs and in run evidence.
- Already clean: Go `t.Run` names (0 of 506), data-testid values, route names, build tags.

**Options the report lists.** The owner settled them on 2026-10-14: fold names into PRD-006 and take A to F, with G as the rule's last line (PRD OPEN-7).

- **A. A naming rule** beside the comment rule (FR-1): names describe behavior, never a plan item. Cost: one skill release, and it depends on agents following it.
- **B. Stop modeling ids in templates.** Reword `skill-template.md:12` and `:72`, `slice.scaffold.md:19` and the proof wording. Qualify or drop the four bare ids. Cost: a skill version bump.
- **C. Map FR to test only in the QA plan.** The proof cell names the test, and the test never names the FR. Cost: `--filter=FR-157` and `-g 'FR-61 pin'` stop working.
- **D. A mechanical check on added names** (FR-3's verb, over non-comment lines outside `docs/`). Cost: an allowlist for factory's domain words.
- **E. Let `/prune` catch them.** Extend `rules.md:21` to plan ids and stage words. Cost: low, since prune runs once per feature.
- **F. Rename what exists.** Factory's share is small: 5 tests, 1 fixture, 1 string, plus the 2 stub strings. cody and arca have far more, archived evidence cites the old names, and migration names cannot change.
- **G. Rename on touch.** Block new id names, and fix old ones when a slice edits the file. Cost: two naming styles side by side for months.

**Still uncertain:**

- How much reading time id names really cost. Few incidents are logged.
- Gitignored run dirs and screenshots were counted on this machine only.
- Whether the rule should cover agent-facing harness feature maps. arca `features/manage.md` has about 100 FR labels.
- Whether arca's shadowed allowlist entries (`internal/audit/writer.go:116-120`) ever rejected a real write.
