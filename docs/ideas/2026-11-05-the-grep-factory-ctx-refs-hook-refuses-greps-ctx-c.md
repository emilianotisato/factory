---
title: "The grep → `factory ctx refs` hook refuses greps ctx cannot answer"
state: "open"
added: "2026-11-05"
origin: "factory upstream"
---

The PreToolUse hook that redirects a bare-word `grep` to `factory ctx refs` costs a round trip in two cases where ctx is the wrong tool:

- **Identifiers that exist only inside string literals.** An XML element name used as `"DirRecep"` in code: `ctx refs` found nothing, and the hook's message did not say a quoted grep was the way.
- **Filtered pipelines.** `grep -n "Name" … | grep -v _test` was refused. The ctx answer was useful, but the refusal cost a turn.

Should: let a grep through when it is piped or path-filtered, or when the pattern is quoted with surrounding quotes; and have the refusal message say "string literals: grep with the quotes".
