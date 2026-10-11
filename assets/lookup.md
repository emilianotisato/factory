Three verbs answer "where is it" in fewer characters than grep and a whole-file read:

- `factory ctx def <symbol>`: where a name is defined, with its body. Example: `factory ctx def createOrder`
- `factory ctx refs <word>`: every use, one line each, comments and strings dropped; `--max` widens. Example: `factory ctx refs OrderStatus --max 30`
- `factory ctx outline <path>`: a file's or a directory's definitions with their lines. Example: `factory ctx outline app/Models`

The one rule: grep only for literal strings and log output.

A recursive `grep` or an `rg` over the repo is answered with a hint naming the verb, and a whole-file read of a file this pack holds with the section it is under: read a range instead.
