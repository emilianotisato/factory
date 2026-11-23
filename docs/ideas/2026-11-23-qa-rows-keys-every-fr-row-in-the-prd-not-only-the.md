---
title: "qa_rows keys every | FR- row in the PRD, not only the QA table's"
state: "open"
added: "2026-11-23"
origin: "PRD-021 polish"
---

Found by the security pass and the second-opinion fallback in PRD-021 polish, rated low. No PRD in this repo has the shape yet.

`pack::qa_rows` (`src/pack.rs`) finds the `Human?` column in the header of the first table holding a `| FR-` row. Its `body` collects `| FR-` rows from the **whole** PRD, though. So a second table whose rows begin `| FR-`, such as a traceability table or a decisions table, would:
- be handed as QA rows that owe evidence;
- shift the `FR-n.k` numbering of the real rows;
- have its `Human?` cell read from the wrong column.

An escaped `\|` inside a cell shifts the columns the same way.

**Shape:** take the body from the contiguous table that starts at `first`, the same bound the header already uses since AD-2, and split cells on unescaped pipes only. **Test:** a PRD with a `| FR-` row in a table above the QA plan.
