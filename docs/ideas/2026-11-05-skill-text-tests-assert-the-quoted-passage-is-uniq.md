---
title: "Skill-text tests: assert the quoted passage is unique"
state: "open"
added: "2026-11-05"
origin: "harvest 2026-11-05"
---

From PRD-012's retro. Skill-text tests (`tests/skills_bundle.rs` and friends) assert that a skill file contains a needle. Three of them passed after the rule they guard was deleted, because another line in the same file held the same words. For prose FRs these tests are the gate's only proof, and they were the weakest tests in the diff.

Shape: a helper such as `assert_unique_passage(file, needle)` that fails when the needle occurs zero times or more than once, used by every skill-text test. `/slice` asks each skill-text test to quote the passage, not a word it shares with others. Acceptance: deleting the guarded sentence turns its test red.
