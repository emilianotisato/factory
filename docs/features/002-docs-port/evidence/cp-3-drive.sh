#!/usr/bin/env bash
# CP-3 of PRD-002: drives the FR-15 and FR-17 rows on the installed binary
# (~/.cargo/bin/factory, put there by slice 11's `cargo install --path .`),
# one transcript per row.
set -u
SRC=/home/emiliano/Projects/factory
EV=$SRC/docs/features/002-docs-port/evidence
BIN=$HOME/.cargo/bin/factory
mkdir -p /tmp/cp3
W=$(mktemp -d /tmp/cp3/work.XXXX)
mkdir -p "$W/data" "$W/config" "$W/claude"
: > "$W/gitconfig"
export FACTORY_DATA_DIR=$W/data FACTORY_CONFIG_DIR=$W/config CLAUDE_CONFIG_DIR=$W/claude
export GIT_CONFIG_GLOBAL=$W/gitconfig GIT_CONFIG_NOSYSTEM=1
export PATH=$HOME/.cargo/bin:/usr/bin:/bin

# `t cmd...`: echo the command, run it, print its exit code.
t() {
  echo "\$ $*"
  "$@" 2>&1
  echo "[exit $?]"
  echo
}
# `ti body cmd...`: the same, with `body` on stdin.
ti() {
  local body=$1; shift
  echo "\$ printf '$body' | $*"
  printf '%s\n' "$body" | "$@" 2>&1
  echo "[exit $?]"
  echo
}
f() { sed "s#$W#\$W#g"; }
newrepo() {
  git init -q -b main "$1"
  git -C "$1" config user.name Fixture
  git -C "$1" config user.email fixture@example.invalid
}
commit() { git add -A && git commit -qm "$1"; }

fr15() {
  echo "# FR-15 — factory docs: the verb set in v3 (CP-3, $(date -I))"
  echo "# binary: $(command -v factory)"
  echo
  echo "## factory docs --help, in a dir with no docs/"
  cd "$W"
  t factory docs --help
  echo "--- verb lines: $(factory docs --help | sed -n '/^Commands:/,/^$/p' | grep -c '^  [a-z]')"
  echo
  echo "## each verb once in a v3 repo built like the harness fixture"
  newrepo "$W/v3"
  cd "$W/v3"
  t factory docs init
  t factory docs new fixture --title Fixture
  t factory docs slice 001 a --title A
  t factory docs slice 001 b --title B
  t factory docs slice 001 c --title C --depends 2
  commit fixture
  t factory docs status
  t factory docs status 001 --json
  t factory docs set 001 status=ready
  ti 'Picked the cheap one.' factory docs log add 001 --kind decision --title 'Which one?' --slice 1 --flag fyi --body-file -
  t factory docs log show 001
  t factory docs gate set true
  t factory docs gate
  t factory docs gate --inner
  t factory docs lanes
  t factory docs lanes set 1 --env PORT=8001
  commit setup
  t factory docs preflight 001
  ti 'A probe idea.' factory docs idea add --title Probe --body-file -
  t factory docs idea list --json
  id=$(factory docs idea list --json | jq -r '.[0].id // .ideas[0].id')
  commit idea
  t factory docs idea drop "$id" --because probe
  t factory docs retro 001
  t factory docs check
  t factory docs doctor
  t factory docs new other --title Other
  commit other
  t factory docs renumber 002 007
  commit renumber
  t factory docs set 001 slices.1.status=done slices.2.status=done slices.3.status=done
  t factory docs ship 001
  commit ship
  t factory docs archive 001
  commit archive
  t factory docs migrate --dry
  t factory docs hook
  t factory docs status
  echo "## the removed verbs exit 2 in a v3 repo"
  t factory docs render --write
  t factory docs vendor
  t factory docs log merge 001
}

fr17() {
  echo "# FR-17 — coexistence: the shim, the hook body, the skills (CP-3, $(date -I))"
  echo "# binary: $(command -v factory)"
  echo
  echo "## the shim: a v2 fixture whose .docs/bin/docs.mjs is the 3-line stub"
  cp -r "$SRC/tests/fixture/v2" "$W/v2"
  cp "$SRC/tests/fixture/stub-docs/docs.mjs" "$W/v2/.docs/bin/docs.mjs"
  newrepo "$W/v2"
  cd "$W/v2"
  commit fixture
  t cat .docs/bin/docs.mjs
  t factory docs status 001 --json
  t factory docs migrate --dry
  echo "## native in v3: the same call prints the board"
  newrepo "$W/hook"
  cd "$W/hook"
  factory docs init >/dev/null
  mkdir -p docs/archive/001-x
  echo '# Old' > docs/archive/001-x/prd.md
  factory docs new fixture --title Fixture >/dev/null
  commit fixture
  t factory docs status 002 --json
  echo "## the hook body: installed, then three commits"
  t factory docs hook
  t head -c 60 .git/hooks/pre-commit
  echo; echo
  t cat .git/hooks/pre-commit
  echo "--- case 1: modify a file under docs/archive/ and commit"
  echo 'edit' >> docs/archive/001-x/prd.md
  git add -A
  before=$(git rev-parse HEAD)
  t git commit -m 'edit the archive'
  echo "--- HEAD unchanged: $([ "$(git rev-parse HEAD)" = "$before" ] && echo yes || echo NO)"
  git reset -q && git checkout -q -- docs/archive
  t git status --short
  echo
  echo "--- case 2: a broken link in the feature's prd.md, factory on PATH"
  t command -v factory
  echo 'See [gone](missing.md).' >> docs/features/002-fixture/prd.md
  git add -A
  t git commit -m 'broken link'
  echo "--- HEAD unchanged: $([ "$(git rev-parse HEAD)" = "$before" ] && echo yes || echo NO)"
  echo
  echo "--- case 3: the same staged change, PATH holding only the git and sh dir"
  (
    export PATH=/usr/bin
    t command -v factory
    t git commit -m 'broken link, no factory'
  )
  echo "--- HEAD moved: $([ "$(git rev-parse HEAD)" != "$before" ] && echo yes || echo NO)"
  echo
  echo "## the skills: every .md under ~/.claude/skills naming docs.mjs"
  t command grep -rn 'docs\.mjs' "$HOME/.claude/skills/" --include='*.md'
  echo "--- the old command spelling, which must be gone:"
  t command grep -rn 'node ~/.claude/skills/feature-docs/bin/docs.mjs' "$HOME/.claude/skills/" --include='*.md'
  t git -C "$HOME/.claude/skills" diff --stat
  t git -C "$HOME/.claude/skills" status --short
  echo "## factory docs check in this repo (still v2), through the shim"
  cd "$SRC"
  t factory docs doctor
  t factory docs check
}

fr15 2>&1 | f > "$EV/cp-3-fr15.txt"
fr17 2>&1 | f > "$EV/cp-3-fr17.txt"
echo "work dir: $W"
