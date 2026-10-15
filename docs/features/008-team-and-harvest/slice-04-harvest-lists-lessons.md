# Slice 04 — factory docs harvest lists a batch's lessons

**Source**: [`prd.md`](prd.md) → §3 FR-5 (the read). **Depends on**: — (independent; slice 05 adds `--done` to the verb this slice creates)
**Created**: 2026-10-15

**Progress**: built 2026-10-15.
- **Built**: `src/docs/harvest.rs` (the read: frontmatter and H1 dropped, `## Numbers` cut to the next `## `, ideas matched on a whole `PRD-NNN` token; a feature with no `retro.md` gets one line), the `harvest` verb in `src/cli/docs.rs`, the help test at nineteen verbs, three tests in `tests/docs_lifecycle.rs` on a fixture with 001 and 002 shipped and 003 ready.
- **Inner loop**: 2 runs, 1 red, about 2.5 min. The red run caught a `cargo fmt` miss in `harvest.rs`.
- **Deviations**: none. AD-1 (fyi): an origin naming another repo's `PRD-NNN` still matches.
- **Prove** (left for the checkpoint): FR-5 read half. `factory docs harvest 007` on this repo printed 007's retro without the Numbers block and its 5 ideas; `harvest 008` exits 2, "PRD-008 is ready, not shipped".

**Slice**: After this slice, `factory docs harvest [NNN…]` prints a batch's lessons and changes nothing: each feature's retro sections without the Numbers block, then the ideas whose origin names that feature. With no numbers the batch is every shipped feature. The moves (`--done`, and `docs archive` sharing them) are slice 05's; the routing is slice 08's `/harvest` skill.

## Current code / environment

- **The verb enum**: `enum Verb` (`src/cli/docs.rs:22-117`), dispatched from `:347` on (`Verb::Ship` and `Verb::Archive` at `:378-379`). An `Error::Msg` exits 2. Each verb's doc comment is its one help line.
- **The help test**: `DOCS_VERBS: [&str; 18]` (`tests/docs_v3.rs:318-337`) and `help_lists_the_eighteen_verbs_in_and_out_of_a_repo` (`:356`); `help_verbs` (`:340`) fails a wrapped help line. Adding a verb makes it 19 and renames the test.
- **The batch**: `board::scan(root)` (`src/docs/board.rs:135`) returns `live` and `archived` features (`:54-58`); `Feature::status()` (`:66-80`) says `archived` for anything under `docs/archive/`, whatever its state says. `Board::resolve` (`:240-252`) takes a number, a slug or a folder name.
- **The retros**: every one opens `# Retro — PRD-NNN`. 001 to 003 carry YAML frontmatter above it, and prose between the H1 and the first `##`. The Numbers block is a `## Numbers` section wrapping `<!-- factory:numbers -->` … `<!-- /factory:numbers -->` (`src/retro.rs:13-14`); 003's retro has none. 001 also has `### Numbers` subsections inside its two A/B sections (`docs/features/001-driver/retro.md:95,179`): those are lessons, keep them.
- **The ideas**: `ideas::list(root)` (`src/docs/ideas.rs:280`) returns rows with `id`, `origin: Option<String>` and `first_sentence` (`:49-60`). Origins here read `"PRD-007 polish"` or `"factory PRD-007 run 007-20261015-c8fa"`; 22 of the 44 ideas have none. `2026-10-15-friction-cody-agent-prd-026` names another repo's PRD-026: matching by number alone mismatches it in a repo that has a 026.
- **The fixture**: `Fixture::new` (`tests/harness`) gives one live feature, `001-fixture`, with three slices and no retro. Nothing builds two shipped features yet: set 001's slices done and `docs ship 001`, then `docs new` a second and `docs ship` it (`ship` accepts a feature with no slices, `src/docs/ship.rs:27-42`). `commit(fx)` is at `tests/docs_lifecycle.rs:23-26`.
- **Deliberate non-changes**: *(prd §4 OPEN-3 "The owner's call, no threshold or nudge.")* No count, age or reminder decides a batch.

## Tasks

### 1. The harvest read, in the library — high — blocked by: —
**Where**: a new `src/docs/harvest.rs`, registered in `src/docs/mod.rs`.

**Source**: §3 FR-5: "**`factory docs harvest [NNN…]`** prints the batch's lessons, read-only. For each feature: its retro's sections under `# Retro`, without the Numbers block, then each idea in `docs/ideas/` whose `origin` names `PRD-NNN` (`"PRD-007 polish"`, `"factory PRD-007 run …"`), as its id and first sentence. With no numbers, the batch is every `shipped` feature; there is no threshold or nudge, the batch is the owner's call. A named feature that is not `shipped` is refused with its status."

One function takes the root and the references and writes the text. Per feature: a header naming `PRD-NNN` and its folder, the retro text after the H1 with the `## Numbers` section cut to the next `## ` or the end, then the ideas, one line each with the id and the first sentence. Resolve named features through `Board::resolve` and refuse one whose `status()` is not `shipped`, naming the status. With no references, take the live features whose status is `shipped`, in number order.

The trap: match `PRD-NNN` in the origin as a whole token, so `PRD-0070` and the text `PRD-007x` do not match. A feature with no `retro.md` gets one line saying so, not an error: a harvest of a feature shipped by hand still lists its ideas.

**Acceptance:** task 3's test.

### 2. The verb — high — blocked by: 1
**Where**: `src/cli/docs.rs:22-117,347+`, `tests/docs_v3.rs:318-356`.

**Source**: §3 FR-5: "**`factory docs harvest [NNN…]`** prints the batch's lessons, read-only."

`Harvest { features: Vec<String> }` with a one-line doc comment, dispatched to task 1. `DOCS_VERBS` gains `harvest` and the help test's name says nineteen.

**Acceptance:** the help test passes with 19 verbs.

### 3. The test — high — blocked by: 2
**Where**: `tests/docs_lifecycle.rs`.

**Source**: §3 FR-5 acceptance: "In a fixture with two shipped features, `factory docs harvest` prints both retros' sections and the ideas whose origin names them."

Two shipped features, each with a `retro.md` holding a lesson section and a `## Numbers` block; one idea whose origin names `PRD-001`, one naming `PRD-0012`, one with no origin. Then:

- `factory docs harvest` prints both lesson sections, no numbers marker, and the `PRD-001` idea's id and first sentence under 001 only;
- `factory docs harvest 002` prints 002 alone;
- a third feature left `ready` and named: exit 2, the message names `ready`.

**Acceptance:** the three cases above, in tests named for the behavior.

### 4. Verify — medium — blocked by: 1, 2, 3
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 5. Prove — at the checkpoint — blocked by: 4
Rows from [`prd.md`](prd.md) §5 this slice proves, below the UI:

- **FR-5** (the read half): "run: `factory docs harvest` … on a fixture with two shipped features" — task 3's test transcript, and `factory docs harvest 007` on this repo. The `--done` half is slice 05's.

**Acceptance:** after the checkpoint, Progress names the transcript.
