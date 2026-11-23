---
title: "Let a PRD authorize a test change, provably, without loosening Never"
state: "open"
added: "2026-11-23"
origin: "PRD-021 interview"
---

**For PRD-022** (guards a branch can't loosen). Raised at PRD-021's interview, 2026-11-23, when FR-1 turned out to need a live gate test replaced.

**What happens.** `assets/autonomy.md:62` puts "skip, delete or weaken a test this run did not write" under **Never**, and the Never items have no "unless the PRD says so" escape: "A change the run believes one of these needs is a **block**". So no PRD can order a test change, and a feature that deliberately changes behaviour an older test pins cannot be built unattended at all. PRD-021 hit exactly this: removing the high-water branch in `covered()` (`src/docs/board.rs:338-346`) requires inverting `a_log_without_covers_keeps_the_high_water_rule` (`tests/docs_log.rs:289-308`), a test the run would not own. PRD-021 took the route that avoids the test instead, and the false-green path it leaves is now its own follow-up.

**Why the rule is right as far as it goes.** It stops the one cheat that matters: an agent breaks real behaviour, a test it does not own goes red, and it weakens that test to go green. The rule should not also freeze every test a deliberate spec change touches.

**What tells the two apart, deterministically.** A **weakened** test still passes against the *old* code, because it asserts less than before. A **legitimate** replacement pins behaviour the old code does not have, so it fails against the old code. So: run the branch's version of each changed test against the merge base.

- Passes on base → it was weakened. Block.
- Fails on base → it pins something new. Allow.
- Deleted, with no replacement to run → always block.

**Why that is not enough alone.** It still lets an agent rewrite any test it likes, as long as the new assertion is red on base. So pair it with an authorization the branch cannot grant itself: the PRD names the tests the feature may change, and the driver reads that list **from the merge base commit**, the same way PRD-022 wants the leaks allowlist read. The `Never` line then reads: unless the agreed list names it **and** the replacement is seen red against the base.

**Cost.** Checking a test against the base means building the base tree: minutes in Rust, seconds in PHP or JS. It should run only when a test the run does not own actually changed, at a checkpoint and in polish, never in the inner loop.

**Minimal shape.** A PRD lists `tests/docs_log.rs::a_log_without_covers_keeps_the_high_water_rule` as changeable. The branch inverts it. At the checkpoint the driver builds the base, runs the branch's version of that test against it, sees it red, and allows the change. A branch that instead deletes an assertion from an unlisted test blocks, and one whose rewrite passes on base blocks with "that test was weakened".
