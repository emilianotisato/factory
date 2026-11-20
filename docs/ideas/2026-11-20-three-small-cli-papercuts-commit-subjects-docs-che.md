---
title: "Three small CLI papercuts: commit subjects, docs check scope, leaks check"
state: "open"
added: "2026-11-20"
origin: "factory upstream"
---

**1. Builder commit subjects cut mid-phrase.** A slice commit read `feat(042): slice-05 Numbering continuity: marked`: the subject was truncated at a character count, not a word. Cap the subject at 50 characters on a word boundary, or shorten the slice title.

**2. `factory docs check` takes no feature argument,** while `docs status NNN` and `docs log NNN` do. Checking one feature means reading the whole board's output.

**3. The leaks check misses plan labels that are not numbered.** It catches `PRD-n` and `FR-n`, but not a phase label in a test name (`test('each day-one effect …')`) or a test file named after the PRD's title (`…PapercutsTest.php` for a PRD titled "papercuts"). Both were caught only by a human-style review. Consider matching the PRD title words and phase labels from the plan.
