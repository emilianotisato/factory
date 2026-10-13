# factory

A feature goes in as a conversation. A reviewed, green pull request comes out. The binary runs the loop in between, and the model is spent only where judgment is needed.

`factory` is a Rust CLI that drives unattended feature delivery with coding-agent workers. You write the PRD with the agent, in an interview. The binary slices it, spawns one worker per slice, runs the gate, commits, runs checkpoints, polishes, watches CI and opens a draft PR. You come back to a short list of decisions it made alone, answer them from your phone, and merge.

This README describes where the project is going, not only where it stands today. Where it stands is at the bottom.

## Why

The first version of this workflow was a family of Claude Code skills and a 2,000-line Node script. It worked, and it shipped real features in a dozen repos, but it paid for itself three times over:

- **Token hungry.** The orchestrator was a model session that re-read the board, the skill and the log on every turn. One feature cost 42M, 73M and then 135M cache-read tokens. A quarter of every run went to orchestration, not to code.
- **Slow.** Every gate run, every `git` call, every log merge cost a model turn to decide and another to read the result. Builds were serial: one feature took 254 build minutes for its first 12 slices.
- **Messy.** Twelve projects carried copies of the same skills. The rules lived in prose a model had to re-derive every run.

The inversion is the whole idea. **The binary orchestrates; the model only builds, reviews and decides.** Running a gate, committing by pathspec, merging a log, watching CI, opening a PR, pausing on quota: none of these need intelligence, so none of them cost tokens. The savings are fixed per run, and the builders get a clean, small context instead of a 170k-token session at the compaction ceiling.

## The loop

1. **Idea.** One file under `docs/ideas/`. Ideas have a state, an origin and a re-trigger ("look again when X happens"). A cross-repo scanner shows the open ideas of every project you own.
2. **PRD.** `/prd` interviews you, checks every claim against the real code, and writes `docs/features/NNN-slug/prd.md`: why now, blast radius, one section per functional requirement with the decisions taken and the alternatives rejected, a QA plan, what is not handled. An adversarial pass from a second vendor hunts for holes before you agree to it.
3. **Slices.** The PRD is cut into vertical slices, each one buildable and provable on its own, with a dependency graph so independent slices build in parallel.
4. **`factory run NNN`.** A detached driver takes the feature. For each ready slice it builds a context pack (the slice, the PRD sections it cites, a code outline, the contract) and spawns a worker, `claude -p` today. Workers run in lanes: isolated working copies with their own database, ports and env, so parallel builds never collide. Every slice ends with a commit the driver makes.
5. **Checkpoints.** Every N slices the driver runs the full gate on what HEAD actually holds, spawns a fix worker on red, and a checkpoint worker that proves the requirements covered so far against the real app.
6. **Blocks.** A worker that hits a real fork stops and asks. The lane parks, the other lanes keep going, and the question reaches you as a notification. `factory answer BLOCK-n "..."` resumes it. No work is lost when the session that started the run dies: the driver is a process, not a chat.
7. **Polish.** A local code review, a security pass, a UX pass over the changed screens, a second opinion from a different vendor, then test cleanup. Findings are triaged into fix, ask or park, and the fixes are made.
8. **Ship.** Closeout writes the retro, opens a draft PR, watches CI and fixes red rounds. It never merges.
9. **Review.** The run kept a log of every decision it made alone, each flagged `decide`, `fyi` or `no`. The cockpit walks you through the `decide` ones, four at a time, with the alternative and the cost of undoing. A changed answer becomes a commit on the branch. Then you merge.
10. **Learn.** Every run writes numbers to a local metrics store: tokens per role, per worker, per subagent; what each pack was made of; gate reds, CI reds, findings by severity, decisions and blocks. The retro compares the run with the last three on the same repo. Overturned decisions, repeated reds and parks queue up as events, and the learn loop proposes changes to the rules with a hypothesis each, checked after N runs. It explains and asks. It never edits a rule unattended.

## The pieces

**The docs framework.** `docs/project.toml` holds the project's knobs: the gate, the inner loop, lane count, model and effort per role, a per-run token budget. Each feature folder has `prd.md`, its slices, a `state.toml` written only by the binary under a lock, an append-only `log.jsonl` of decisions, blocks and notes, and a `retro.md`. `factory docs check` validates the board and runs as a pre-commit hook. `factory docs migrate` moves a repo from the previous format in one reviewable diff.

**The skills.** The judgment calls stay in Claude Code as thin skills, shipped from this repo and installed by `factory` into each project: `/prd`, `/slice`, `/deliver` (the cockpit), `/polish`, `/gate`, `/harness`, `/prune`, `/ux`, `/lanes`. Project skills are rendered from a profile plus overlays, never copied, and drift is detected by hash. Three scopes with promotion: project, user, base.

**The gate.** `/gate` interviews a project into one command that proves a change is done: tiered checks for lint, types, coverage floors, complexity, mutation, size budgets and architecture rules, measured against reality and proven to fail before they are declared. Every slice's Verify task, every checkpoint and the polish pass run that one command. A fast subset is the inner loop.

**The app harness.** `/harness` writes a project-local skill that drives the real application the way a user does, in a browser, a CLI or an API, and captures proof. Checkpoint workers use it, so "done" means "seen working", not "tests pass".

**Lanes.** A lane is a git worktree with its own copy of ignored state (a copy-on-write clone where the filesystem allows it) and a per-lane setup: database, ports, queue prefix, env. `/lanes` interviews a project into those setups and proves two lanes' suites run at once without colliding. The driver assigns a lane per worker, lands each slice on the feature tip, and spawns a merge worker on a conflict.

**`factory ctx`.** Code lookups for workers: definitions, references and outlines over ripgrep and tree-sitter, no language server. Lookups were 56 to 74% of a worker's tool calls and 92% of the bytes it read; a precomputed outline in the pack and a cheap `def` cut both.

**The cockpit.** `factory wait`, `status`, `answer`, `stop`. A thin Claude Code session, phone-sized output, that relays blocks and runs the closing review. If the session dies, `factory wait` replays what it missed.

**The observatory.** SQLite in the user data dir. `factory report` per run and per worker, `factory retro` writes the numbers into the feature's retro. Experiments are knob changes in `project.toml`, one variable per feature, with a stated hypothesis, measured here. No A/B against anything else.

**Stacks as data.** The core is stack-blind. Laravel/Vue, Go and Rust are the dogfood stacks, each a manifest plus markdown with named prompt slots. Other agents as workers (Codex, Cursor) are the same shape: a different command and stream parser, nothing else.

## What it never does

- Merge, force-push, push to the default branch, delete a branch, or close a PR. Those are yours. Hooks enforce it, not prose.
- Edit a rule, a skill or a gate unattended. The learn loop proposes; you decide.
- Pretend. A slice with no proof is not done. A checkpoint that cannot name the requirements it proved does not close. A finding that was parked says so in the PR.

## Where it stands

Early and green. The driver (PRD-001), the metrics store (PRD-003) and the docs port (PRD-002) are built and merged, and the binary has delivered three features in this repo and two in production Laravel repos. Workers are Claude Code only. Lanes share a working tree today; isolated lanes are the next driver PRD. The skills still live outside this repo; moving them in with an installer is PRD-004. The learn loop, stack plugins and a bug inbox fed by production errors are ideas with a shape, not code.

It is built for one developer so far. What a team repo commits, and what stays on the machine that ran it, is an open decision.

## Trying it

You need Rust, the `claude` CLI logged in, and `gh` authenticated.

```
cargo install --path .
factory --help
factory docs init        # in a project: project.toml, index.md, features/, ideas/, archive/
factory docs gate set "<the command that proves a change is done>"
```

Then `/prd` in Claude Code to write the first feature, and `/deliver NNN` to build it.

The gate for this repo is `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`. The test harness is a fixture repo plus a stub `claude` that replays recorded streams, so the whole loop runs in `cargo test` with no tokens.
