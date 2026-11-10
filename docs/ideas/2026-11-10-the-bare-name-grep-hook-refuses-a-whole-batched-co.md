---
title: "The bare-name grep hook refuses a whole batched command"
state: "open"
added: "2026-11-10"
origin: "factory upstream"
---

**What happens.** The PreToolUse hook that redirects a bare-word `grep` (a class or symbol name) to `factory ctx` inspects the full Bash command. When an agent sends one command that chains several steps (`cd … && grep -n Foo app/ && sed -n 1,40p bar.php && …`), one matching `grep` segment makes the hook refuse the entire command. The agent loses the other steps, re-plans, and resends them, often split into several calls.

**How often.** Reported in three of seven feature retros in one project over one week, each during polish triage, each costing at least one extra round trip and a broken batch.

**What it should do.** Act on the matching segment only. Either:

- rewrite that segment to the `factory ctx refs` equivalent and let the rest of the command run, or
- let the command run and append a note to the output suggesting `factory ctx` for that symbol next time.

Refusing the whole command should be kept for a command that is only the matching grep.

**Minimal shape.**

```
cd repo && grep -rn SomeClass app/ && sed -n 1,20p app/Other.php
```

Today: refused. Expected: the `sed` runs; the grep is rewritten or annotated.
