Three verbs answer "where is it" in fewer characters than grep and a whole-file read:

- `factory ctx def <symbol>`: where a name is defined, with its body when one definition matches. Example: `factory ctx def createOrder`
- `factory ctx refs <word>`: every use, one line each, comments and strings dropped; `--max` widens. Example: `factory ctx refs OrderStatus --max 30`
- `factory ctx outline <path>`: a file's or a directory's definitions with their lines. Example: `factory ctx outline app/Models`

The one rule: a bare name goes to ctx, grep anything else: a literal string, a regex, several patterns, log output. `--vendor` takes `refs` and `def` into vendor code.

A recursive `grep` or an `rg` for one bare name over the repo's own code is answered with a hint naming the verb, and a whole-file read of the slice file with a hint: the pack carries it in full under `# Slice`, so read a range if you need more. Any other file reads whole.
