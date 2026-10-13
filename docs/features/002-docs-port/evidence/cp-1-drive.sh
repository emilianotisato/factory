#!/usr/bin/env bash
# CP-1 of PRD-002: drives the FR-13, FR-14, FR-17 (routing half) and FR-19
# (store half) rows on the branch binary, writing one transcript per row.
set -u
SRC=/home/emiliano/Projects/factory
EV=$SRC/docs/features/002-docs-port/evidence
BIN=/mnt/devcache/emiliano/cargo-target/debug/factory
INSTALLED=/home/emiliano/.cargo/bin/factory
W=$(mktemp -d /tmp/cp1/work.XXXX)
mkdir -p "$EV" "$W/data" "$W/config" "$W/claude"
: > "$W/gitconfig"
export FACTORY_DATA_DIR=$W/data FACTORY_CONFIG_DIR=$W/config CLAUDE_CONFIG_DIR=$W/claude
export GIT_CONFIG_GLOBAL=$W/gitconfig GIT_CONFIG_NOSYSTEM=1
export FACTORY_STUB_DIR=$W/stub PATH=$SRC/tests/stubs:$PATH
mkdir -p "$W/stub"

# `t cmd...`: echo the command, run it, print its exit code.
t() {
  echo "\$ $*"
  "$@" 2>&1
  echo "[exit $?]"
  echo
}
f() { sed "s#$W#\$W#g"; }
newrepo() {
  git init -q -b main "$1"
  git -C "$1" config user.name Fixture
  git -C "$1" config user.email fixture@example.invalid
  cp "$SRC/tests/fixture/gate.sh" "$1/gate.sh"
}

# ---------- v3 repo, shaped like Fixture::v3 ----------
V3=$W/v3
newrepo "$V3"
cd "$V3"
$BIN docs init >/dev/null
printf '[docs]\nframework = 3\ngate = "./gate.sh"\ninner = "./gate.sh --inner"\nlanes = 2\ngate_parallel = true\n' > docs/project.toml
$BIN docs new fixture --title Fixture >/dev/null
for s in a b; do $BIN docs slice 001 $s >/dev/null; done
$BIN docs slice 001 c --depends 2 >/dev/null
$BIN docs set 001 status=ready >/dev/null
printf '/scenario/\n' > .gitignore
git add -A && git commit -qm 'chore: fixture repo'

{
  echo "# CP-1 FR-13 — branch binary $( $BIN --version ), v3 repo built like Fixture::v3"
  echo
  t $BIN docs status
  t $BIN docs status 001 --json
  t $BIN docs set 001 slices.3.status=building
  t grep -A2 '^\[slices.3\]' docs/features/001-fixture/state.toml
  echo '$ factory docs status 001 --json | jq .slices[2].status'
  $BIN docs status 001 --json | jq .slices[2].status; echo
  t $BIN docs set 001 status=later
  echo "--- board prose deleted from docs/index.md (first line kept) ---"
  $BIN docs status > ../before.txt; $BIN docs status 001 --json > ../before.json
  head -1 docs/index.md > ../index.md && cp ../index.md docs/index.md
  t wc -l docs/index.md
  $BIN docs status > ../after.txt; $BIN docs status 001 --json > ../after.json
  t diff ../before.txt ../after.txt
  t diff ../before.json ../after.json
  git checkout -q docs/index.md
  echo "--- project.toml with [docs], [run] and [workers.polish] (v3 repo) ---"
  printf '\n[run]\nbudget = 5000000\n\n[workers.polish]\nmodel = "sonnet"\n' >> docs/project.toml
  t cat docs/project.toml
  t $BIN run 001
  git checkout -q docs/project.toml
} 2>&1 | f > "$EV/cp-1-fr13.txt"

# ---------- v2 repo with the stub docs.mjs ----------
V2=$W/v2
newrepo "$V2"
cd "$V2"
cp -r "$SRC/tests/fixture/.docs" .docs
node .docs/bin/docs.mjs init >/dev/null
node .docs/bin/docs.mjs new fixture --title Fixture >/dev/null
node .docs/bin/docs.mjs slice 001 a >/dev/null
git add -A && git commit -qm 'chore: v2 fixture'

{
  echo "# CP-1 FR-13 warning half on the v2 harness + FR-17 routing half"
  echo
  echo "--- v2 repo, docs/project.toml with [docs], [run] and [workers.polish] ---"
  printf '[docs]\nlanes = 2\n\n[run]\nbudget = 5000000\n\n[workers.polish]\nmodel = "sonnet"\n' > docs/project.toml
  t cat docs/project.toml
  t $BIN run 001
  echo "--- control: an unknown [deploy] table does warn ---"
  printf '\n[deploy]\nfoo = 1\n' >> docs/project.toml
  t $BIN run 001
  rm docs/project.toml
  echo "--- FR-17: v2 repo, .docs/bin/docs.mjs replaced by the stub ---"
  cp "$SRC/tests/fixture/stub-docs/docs.mjs" .docs/bin/docs.mjs
  t cat .docs/bin/docs.mjs
  t $BIN docs status
  t $BIN docs status 001 --json
  t $BIN docs render --write
  echo "--- FR-17: v3 repo is native (no node, removed verbs refused) ---"
  cd "$V3"
  t $BIN docs status 001
  t $BIN docs render --write
  t $BIN docs log merge 001
} 2>&1 | f > "$EV/cp-1-fr17.txt"

# ---------- FR-14: the log ----------
cd "$V3"
{
  echo "# CP-1 FR-14 — the log, in the v3 repo"
  echo
  echo "--- two processes append 50 entries each ---"
  for w in 0 1; do
    ( for i in $(seq 0 49); do
        printf 'worker %s entry %s\n' $w $i | $BIN docs log add 001 --kind decision --title "w$w-$i" --body-file - >> ../ids-$w.txt || echo "FAIL w$w i$i"
      done ) &
  done
  wait
  L=docs/features/001-fixture/log.jsonl
  t wc -l $L
  echo "first id: $(head -1 $L | jq -r .id) — last id: $(tail -1 $L | jq -r .id)"
  echo "distinct ids: $(jq -r .id $L | sort -u | wc -l), gaps against AD-1..AD-100: $(comm -3 <(jq -r .id $L | sort) <(seq 1 100 | sed 's/^/AD-/' | sort) | wc -l)"
  echo "ids printed per process: $(wc -l < ../ids-0.txt) + $(wc -l < ../ids-1.txt)"
  t git status --porcelain
  echo "--- a body with a title line and preamble round-trips ---"
  printf '# A title\n\nprose before any heading\n\n## A heading\n\ntext' > ../body.md
  t $BIN docs log add 001 --kind decision --flag decide --title "round trip" --body-file ../body.md
  echo '$ factory docs log show 001 --json | jq -r ".[] | select(.id==\"AD-101\") | .body" | diff - body.md'
  $BIN docs log show 001 --json | jq -j '.[] | select(.id=="AD-101") | .body' | diff - ../body.md; echo "[exit $?]"; echo
  t $BIN docs log add 001 --kind decision --flag fyi --title "an fyi one"
  t $BIN docs log add 001 --kind block --phase checkpoint --title "a red checkpoint"
  t $BIN docs log show 001 --decide
} 2>&1 | f > "$EV/cp-1-fr14.txt"
cd "$SRC" && cargo test -q --test docs_log 2>&1 | grep -v '^\s*Compiling\|Finished\|Running' >> "$EV/cp-1-fr14.txt"

# ---------- FR-19 store half ----------
S=$W/store
mkdir -p "$S"
sqlite3 /home/emiliano/.local/share/factory/factory.db ".backup '$S/factory.db'"
RUN=003-20261013-4139
mkdir -p "$S/runs" && cp -a /home/emiliano/.local/share/factory/runs/$RUN "$S/runs/"

{
  echo "# CP-1 FR-19 store half — a .backup copy of ~/.local/share/factory/factory.db plus a cp -a of runs/$RUN, never the original"
  echo
  t sqlite3 "$S/factory.db" 'SELECT max(version) FROM schema_version'
  cd "$SRC"
  FACTORY_DATA_DIR=$S $INSTALLED report $RUN > "$W/report-installed.txt" 2>&1; echo "installed report exit $?"
  FACTORY_DATA_DIR=$S $INSTALLED report $RUN --json > "$W/report-installed.json" 2>&1; echo "installed report --json exit $?"
  t sqlite3 "$S/factory.db" 'SELECT max(version) FROM schema_version'
  FACTORY_DATA_DIR=$S $BIN report $RUN > "$W/report-branch.txt" 2>&1; echo "branch report exit $?"
  FACTORY_DATA_DIR=$S $BIN report $RUN --json > "$W/report-branch.json" 2>&1; echo "branch report --json exit $?"
  echo "installed: $($INSTALLED --version) — branch: $($BIN --version)"
  t diff "$W/report-installed.txt" "$W/report-branch.txt"
  t diff "$W/report-installed.json" "$W/report-branch.json"
  t cat "$W/report-branch.txt"
  t sqlite3 "$S/factory.db" 'SELECT max(version) FROM schema_version' '.schema retros'
  echo "--- v3 repo: retro --set ---"
  cd "$V3"
  t $BIN docs retro 001 --set score=4
  t $BIN docs retro 001 --set score=high
  t sqlite3 "$FACTORY_DATA_DIR/factory.db" 'SELECT feature, score FROM retros'
  t $BIN docs retro 001
} 2>&1 | f > "$EV/cp-1-fr19.txt"
echo "work dir: $W"
