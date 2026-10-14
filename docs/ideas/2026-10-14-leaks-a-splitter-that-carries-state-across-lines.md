---
title: "leaks: a splitter that carries state across lines"
state: "open"
added: "2026-10-14"
origin: "PRD-006 polish"
---

`leaks::split` reads one line at a time, so it misses everything that spans lines or lives in a syntax it has no row for. Found by the PRD-006 polish reviewers (local, and two of the fallback second-opinion agents).

- A `/* … */` block, a Python docstring, a shell heredoc, a Rust raw string or a JS template literal that crosses a line: the inner lines read as code, so a plan id there is missed, and a quote inside one can open a false string that swallows the rest of the line.
- An unclosed quote on a line swallows the comment after it.
- A leading `* ` in a multiplication continuation line (`    * rate`) reads as a comment line.
- `.vue` and `.blade.php` HTML comments (`<!-- FR-3 -->`) are not comments to the splitter.
- Python `#comment` without a space after code is not split (the `#` rule wants line start or a space before).
- A file whose extension has no `Syntax` row is skipped silently: the report prints no count of skipped files, so a repo in an unlisted language reads `clean`.

Shape of a fix: a small per-file state machine (in-block-comment, in-string with its closer) carried across lines in `scan`, an HTML-comment row for vue/blade, and a `skipped: n files (ext, ext)` line under the comment share. Test against cody-agent and arca-api: today's baselines are 2172 and 1280 fix hits, and each change should move them only by hits a reader agrees are real.
