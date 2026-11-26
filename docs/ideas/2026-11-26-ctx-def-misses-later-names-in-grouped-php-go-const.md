---
title: "ctx def misses later names in grouped PHP/Go const declarations"
state: "open"
added: "2026-11-26"
origin: "PRD-019 polish"
---

**Recon:** `ctx def B` on `public const A = 1, B = 2` and `const KB, MB = …` finds line matches but `confirmed()` requires a tree-sitter `@name` on the symbol; later names in the same declaration are often untagged. Comma-suffix regex alone does not help when the file parses cleanly.
**Next step:** extend PHP/Go tag queries for multi-name `const` specs, or fall back to textual `def` when a line matches `const` patterns but has no tag.
