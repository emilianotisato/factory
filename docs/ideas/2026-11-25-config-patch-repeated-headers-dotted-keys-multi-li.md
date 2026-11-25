---
title: "config::patch: repeated headers, dotted keys, multi-line strings, concurrent writers"
state: "open"
added: "2026-11-25"
origin: "PRD-026 polish"
---

`patched_text` (src/config.rs) edits the first header that normalizes to the table name, and appends a header when none exists. In three cases it refuses a legal file or edits the wrong place:
- a repeated `[review]` header whose key sits under the second one: duplicate-key reject, so the whole batch is dropped (including AD-9's `agent` removal)
- `[workers]` with dotted `build.model = …`: `factory setup workers --role build` appends `[workers.build]`, and the parse rejects the duplicate
- a `"""` multi-line string holding a `key = …` line: the scan can edit inside the string. A removal is not caught by the post-check.
- two `factory answer` / `factory setup` runs at once read, modify and write with no lock, so one change is lost
The post-check keeps the file intact in most of these cases. The fix is a parse-aware edit (toml_edit) or a lock plus a refusal for the shapes the line editor cannot handle.
