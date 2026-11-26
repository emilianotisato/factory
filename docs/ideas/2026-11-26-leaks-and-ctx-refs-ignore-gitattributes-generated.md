---
title: "leaks and ctx refs ignore .gitattributes generated paths that review collect skips"
state: "open"
added: "2026-11-26"
origin: "factory upstream"
---

`factory review collect` and `review second` drop paths that `git check-attr` reports as `linguist-generated` or `linguist-vendored`. Two other verbs do not read the same marks:

- **`factory leaks`** scans every added line, including committed build output. In a repo that commits its bundler output (production has no build step), a minified third-party chunk matches the `id-string` rule on every rebuild, under a new content hash each time. Over 13 consecutive features, every checkpoint and every polish reported the same false hit, and each worker rebutted it by hand. The leaks contract forbids an allowlist entry for it, so the only way out today is rewriting a generated file.
- **`factory ctx refs`**, which the Bash hook sends a bare-identifier `grep` to, walks the committed bundle too. Its root-level skip list covers `build/` and `dist/` only at depth 1, so a nested output dir such as `public/build/` is searched. A name that also appears in a minified chunk prints that whole line, tens of KB, into the worker's context.

**Should:** both verbs honour the same `.gitattributes` marks as review collect, so the repo declares its generated paths once. A path marked generated or vendored is skipped by `leaks` (and reported as skipped, as collect does), and `ctx refs` leaves it out of results unless asked with its vendor flag.

**Minimal shape:**

```
# .gitattributes
public/build/** linguist-generated
```

Commit a minified bundle under `public/build/` containing a string the `id-string` rule matches. `factory leaks --base main` reports it; `factory review collect` skips it. `factory ctx refs <name>` for a name inside that bundle prints the bundle line.

Possible guard for repos that never mark anything: treat a line over a few KB as minified and skip it in both verbs.
