# Slice 02 — The rule, the skill texts, version 0.4.0

**Source**: [`prd.md`](prd.md) → §3 FR-1 (the rule every worker reads), §4 FR-2 (the skill and template edits), §5 FR-3 "Where it runs" item 2 (polish's § 1 Collect step and its two review questions). **Depends on**: — (independent of slice 01: it edits text and one test, and names the verb without calling its code)
**Created**: 2026-10-14

**Progress**:
- Built: `## The plan stays in docs/` in `assets/autonomy.md`, between Never-to-get-green and Tokens, verbatim from the PRD. `src/pack.rs` test `every_role_reads_the_plan_stays_in_docs` builds all six packs (merge included) and asserts the heading once in each; with the heading renamed it fails at the assert, restored it passes. `/slice` trap and Acceptance bullets, `/prd` §QA plan and FR-section bullets, the four anecdotes and the two harness-template lines edited; `/polish` § 1 gains the reader's questions in the local review's prompt and the `factory leaks --base <base-branch>` collector with its exit-code reading and the Fix bucket. Version 0.4.0, `skills/bundle.lock` rewritten with `FACTORY_BUNDLE_WRITE=1 cargo test`. Not tagged.
- Inner loop: 1 run (plus the lock-writing `cargo test` and one targeted red run to prove the test fails without the section), 0 red, about 2 min. `grep -rniE "in a comment|code comment" skills/ assets/` prints nothing.
- Deviations: none. The new `/slice` and `/prd` lines say "never a comment the slice asks for" and "never asks the code to carry a comment", which the grep does not match.
- **Proved at checkpoint 1.** FR-1: the pack test transcript `evidence/cp-1-fr1-pack-test.txt` (`every_role_reads_the_plan_stays_in_docs`, 1 passed).
- **Proved at checkpoint 1.** FR-2: the empty grep, the four anecdote lines, the two harness-template lines and task 3's four `/slice` and `/prd` places as they read now, `evidence/cp-1-fr2-grep-and-lines.txt`. The live `/slice` check waits for the first PRD sliced after the merge.

**Slice**: After this slice every worker's pack carries the section `## The plan stays in docs/`, the shipped skills stop asking for comments and stop modeling plan ids in tests and file names, and `/polish` runs `factory leaks` over the branch and asks a reader's questions of every added comment and name. Every edit under `skills/` in this feature lands here, with the one version bump the bundle lock forces: no later slice touches `skills/`.

## Current code / environment

- **The rule's home**: `assets/autonomy.md` (10,954 bytes) has ten sections: `:9` Who decides what, `:15` The test, `:29` Mapping, `:41` What an unattended run may write, `:49` Never, to get green, `:62` Tokens, `:71` Caps, `:81` Blocker protocol, `:89` The owner answers mid-run, `:93` Resuming. `src/pack.rs:17` embeds it with `include_str!`, and all six packs push it as `# Autonomy`: slice `:188`, fix `:242`, merge `:321`, checkpoint `:423`, polish `:503`, CI fix `:550`.
- **The pack test to extend**: `src/pack.rs:1069` `every_pack_kind_measures_the_sections_it_wrote` builds the slice, fix, checkpoint, polish and CI-fix packs and checks their section names; the merge pack is built in the test above it (`:1011`, `Pack::for_merge(...)`, asserting `"autonomy"` at `:1047`). Slice 04 edits the checkpoint assert of the first test (`:1135-1146`): keep your test a separate function so the two slices do not edit the same lines.
- **The skill lines FR-2 names** (today's text):
  - `skills/slice/SKILL.md:83`, the trap bullet: "**The trap**, when there is one: a fact of today's code … Never a what-if that needs new machinery." `:84` the Acceptance bullet: "a test that could fail. Name what it asserts, not "it works"."
  - `skills/prd/SKILL.md:66`: "… the first builder blocks on it (PRD-001 FR-1 did exactly that)."
  - `skills/prd/SKILL.md:134`, the §QA plan bullet: "… its Proof starts with `run` and names the test or command whose transcript is the evidence."
  - `skills/gate/SKILL.md:132`: "Fmt and clippy at CP-2 of PRD-001 is the pattern." `:134`: "One such test cost PRD-003 a full gate rerun and a Trap lookup each time it fired."
  - `skills/harness/SKILL.md:33`: "… PRD-001's two A/B runs had harness stacks on fixed ports, and the first had to be stopped before the second could drive."
  - `skills/harness/references/skill-template.md:12`, the trigger list: "… "prove FR-n", "screenshot the {feature}" …". `:72`: "Name files by feature id: `rules-create.png`, `rules-create.aria.txt`, `cli-search.txt`."
- **The model to follow**: `skills/gate/SKILL.md:63` names the repo, "cody-agent's PRD-011", and so do `skills/prd/SKILL.md:74,101` ("arca PRD-012", "arca PRD-015 FR-40"). An id read in another repo means that repo's feature, so a bare factory id reads wrong there.
- **Deliberate non-changes in the skill text**: *(prd §4 "Ids stay where they belong. The slice's **Source** line, the PR body table, the log's AD-n and the format examples (`FR-1` in the scaffold, `PRD-013 → 5 slices` in `/slice`) are docs, so they keep their ids.")* So `skills/slice/SKILL.md:68,95-103`, `skills/polish/SKILL.md:164`, `assets/autonomy.md:86`, `skills/prd/SKILL.md:42` and `skills/harness/SKILL.md:9`'s "prove FR-97 in the app" (a request the owner types) stay. `skills/gate/SKILL.md:80` ("PRD-001 of the factory ran `cargo test` alone …") already names the repo: leave it.
- **Polish today**: `skills/polish/SKILL.md:29` is `## 1. Collect — scale the reviewers to the diff`; `:44` local review first, `:46` security-review, `:48` `/ux`, `:52-56` the second opinion and its exit codes ("`0` is findings … `2` is unreviewed … A pass marked `unreviewed` … is never *no findings*"). `:60` is `## 2. Verify and dedup`. The polish worker runs this skill unattended (`assets/contract-polish.md:11`).
- **The version gate**: `Cargo.toml:3` and `skills/bundle.lock` both say `0.3.0`. `tests/skills_bundle.rs:120` rewrites the lock only under `FACTORY_BUNDLE_WRITE=1`, and refuses at an unchanged version (PRD-004's evidence, `cp-2-fr5-2-refused.txt`). So: bump `Cargo.toml` (and `Cargo.lock`'s `factory` entry, which `cargo` updates) to `0.4.0`, then `FACTORY_BUNDLE_WRITE=1 cargo test`, then the inner loop without it. Do not tag: the tag is the owner's, after the merge.
- **No comment ask exists today**: `grep -rniE "in a comment|code comment" skills/ assets/` finds nothing. FR-1's text uses neither phrase, and these edits must not add one.
- **Deliberate non-changes**: *(prd §11 "Not handled: a cleanup skill (OPEN-6) … the report of `factory leaks --all` is the prompt.")* Do not add a skill. *(prd §11 "Not handled: a per-repo rule table".)* Polish runs the verb as it is; it does not add rules of its own.

## Tasks

### 1. The rule in `assets/autonomy.md` — high — blocked by: —
**Where**: `assets/autonomy.md`.

**Source**: §3 FR-1: "Add this section to `assets/autonomy.md`, so every role reads it: build, fix, merge, checkpoint, polish and CI fix all write code." The text is the PRD's block at §3, verbatim:

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

Insert it after `## Never, to get green` (`:49`) and before `## Tokens` (`:62`): it is a rule about what a worker writes, beside the other one. Copy it as the PRD states it, line breaks included; the owner accepted that text.

**Acceptance:** task 2's test.

### 2. Every role reads it — high — blocked by: 1
**Where**: `src/pack.rs` (test module).

**Source**: §3 FR-1 acceptance: "Every role's pack holds the section; a unit test over `pack.rs` checks it. That is the build's proof."

A new test function that builds all six packs, the merge pack included (the setup at `:1011` shows its arguments), reads each pack file and asserts it contains `## The plan stays in docs/` once. Name it for the behavior, e.g. `every_role_reads_the_plan_stays_in_docs`.

**Acceptance:** the test passes with task 1 and fails with the section removed from `assets/autonomy.md`.

### 3. `/slice` and `/prd` never ask for a code comment — high — blocked by: —
**Where**: `skills/slice/SKILL.md:83-84`, `skills/prd/SKILL.md:134` and its §6 FR-section bullet.

**Source**: §4 FR-2: "**`/slice` and `/prd` never ask for a code comment.** An invariant the slice must keep becomes a test that pins it, or a type or a name. The trap is for the builder, not the code." And: "**The QA plan is the only map from FR to test** … The proof row names the test by its behavior name, and the test never names the FR. `skills/prd/SKILL.md:134` says so."

- `/slice`'s trap bullet (`:83`) gains the two sentences: the trap is for the builder, not the code; an invariant the slice must keep becomes a test that pins it, a type or a name, never a comment the slice asks for.
- `/slice`'s Acceptance bullet (`:84`) names the test by the behavior it asserts, never by an FR or slice id.
- `/prd`'s §QA plan bullet (`:134`) says the Proof cell names the test by its behavior name, and that the test never names the FR: the row is the only map from FR to test.
- `/prd`'s §6 "One section per FR" bullet says an FR never asks the code for a comment, with the same invariant line.

**Acceptance:** read back, each of the four places carries its sentence; `grep -rniE "in a comment|code comment" skills/ assets/` still prints nothing (word the new lines without either phrase: the PRD's own "never ask for a code comment" trips it, "never ask the code to carry a comment" does not).

### 4. The harness template stops modeling ids — medium — blocked by: —
**Where**: `skills/harness/references/skill-template.md:12,72`.

**Source**: §4 FR-2: "**The harness template stops modeling ids.** `:12`'s trigger becomes "prove a § QA plan row". `:72` becomes "Name files by the behavior", which its examples already do."

Both edits as stated. The examples on `:72` stay.

**Acceptance:** read back, `:12` holds "prove a § QA plan row" and no `FR-n`; `:72` reads "Name files by the behavior".

### 5. The four anecdotes — medium — blocked by: —
**Where**: `skills/prd/SKILL.md:66`, `skills/gate/SKILL.md:132,134`, `skills/harness/SKILL.md:33`.

**Source**: §4 FR-2: "**The four anecdotes state the fact without the id,** or name the repo the way `skills/gate/SKILL.md:63` does ("cody-agent's PRD-011")." §4 acceptance: "The four anecdote lines and the two harness-template lines read back without a factory id."

Prefer the fact alone: "(factory's first run did exactly that)", "Fmt and clippy red at a checkpoint and never in the inner loop is the pattern", "One such test cost a run a full gate rerun …", "two A/B runs of the same PRD had harness stacks on fixed ports …". Keep an id only where it carries meaning, and then name the repo first, as `:63` does.

**Acceptance:** read back, none of the four lines holds a bare `PRD-n`, `FR-n` or `CP-n`; any id left follows the repo's name.

### 6. Polish runs the check and reads every added comment and name — high — blocked by: —
**Where**: `skills/polish/SKILL.md` § 1 Collect (`:29-58`) and § 2 or § 3 where the Fix bucket is.

**Source**: §5 FR-3, "Where it runs" item 2: "`/polish` runs it over the whole branch in § 1 Collect (`skills/polish/SKILL.md:29`); hits pass § 2 Verify like any reviewer's and land in § 3's Fix bucket, with no question to the owner. Its code review gets one question per surviving added comment, security claims first: does it still match the code beside it, does it sit on the item it describes, does it name only what the tree defines, and would a reader miss it? And one per added name: does it point at the plan without a number, like `weights_match_the_prd_formula` or cody's `seedPre025Workspaces`?" And §5: "A `fix` hit exits 1; a figure is never a hit." And: "Mechanical hits are fixed by the worker; judgment stays with a reader … No new owner question anywhere."

- **A collector paragraph** in § 1, beside the local review: run `factory leaks --base <base-branch>` on every polish, whatever the diff size (it costs no reviewer). Read its exit code the way the second opinion's is read (`:54`): `0` is clean, `1` is hits, anything else means the check did not run, said out loud and never reported as *no hits*. Its hits are findings like any reviewer's: § 2 verifies them and they land in § 3's Fix bucket. Each hit's fix line is its instruction.
- **The reader's questions** go to the local review's prompt, in the same section: one per surviving added comment, security claims first, and one per added name, in the PRD's words above, with its two examples.

**The trap**: polish workers read this skill through `.claude/skills/polish`, a symlink to `skills/polish` in this repo, but run `factory` from `PATH`. This feature's own polish reads the edited text while the installed binary has no `leaks` verb, so the call exits 2 there. The exit-code sentence above is what keeps that from reading as a clean branch; in another repo `factory skills update` installs the skills that match the binary, so the two agree.

**Acceptance:** read back, § 1 names the command, the three exit-code readings and the Fix bucket; the reader's questions are in the local review's prompt with both examples; no line asks the owner a question about a hit.

### 7. Version 0.4.0 and the bundle lock — high — blocked by: 3, 4, 5, 6
**Where**: `Cargo.toml:3`, `Cargo.lock`, `skills/bundle.lock`.

**Source**: §2 Blast radius: "A skill edit means a version bump, a tag and `factory skills update` in each repo."

Bump `version` to `0.4.0`, run `FACTORY_BUNDLE_WRITE=1 cargo test` once to rewrite the lock, then the inner loop without the variable. The tag and `factory skills update` are the owner's, after the merge: do not tag.

**Acceptance:** `skills/bundle.lock` says `version = "0.4.0"` and the bundle tests pass without `FACTORY_BUNDLE_WRITE`.

### 8. Verify — medium — blocked by: 1, 2, 3, 4, 5, 6, 7
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 9. Prove — at the checkpoint — blocked by: 8
Rows from [`prd.md`](prd.md) §9 this slice proves, below the UI:

- **FR-1**: "run the pack test: every role's pack holds `## The plan stays in docs/`. The live-run numbers are observed after ship, on the next real run's retro (FR-4), against the baseline in FR-1" — task 2's test transcript. The live-run numbers are not this run's to prove.
- **FR-2**: "run `grep -rniE "in a comment\|code comment" skills/ assets/`: nothing, as today. Read back the four anecdote lines and the two harness-template lines. `/slice` a small PRD: no slice file asks for a comment or names a test after an FR" — the grep's empty output and the six lines read back. A real `/slice` is attended work and cannot run inside the build, so the build's evidence for the last sentence is the read-back of tasks 3's four places; the first PRD sliced after the merge is the live check.

**Acceptance:** after the checkpoint, Progress holds the pack test transcript, the empty grep and the six lines as they read now.
