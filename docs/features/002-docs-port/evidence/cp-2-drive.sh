#!/usr/bin/env bash
# CP-2 of PRD-002: drives the FR-16, FR-17 (routing half), FR-18 and FR-19
# (closeout half) rows on the branch binary, one transcript per row.
set -u
SRC=/home/emiliano/Projects/factory
EV=$SRC/docs/features/002-docs-port/evidence
BIN=/mnt/devcache/emiliano/cargo-target/debug/factory
GATELOG=/home/emiliano/.local/share/factory/runs/002-20261013-e729/gates/2-2.log
mkdir -p /tmp/cp2
W=$(mktemp -d /tmp/cp2/work.XXXX)
mkdir -p "$EV" "$W/data" "$W/config" "$W/claude" "$W/stub"
: > "$W/gitconfig"
export FACTORY_DATA_DIR=$W/data FACTORY_CONFIG_DIR=$W/config CLAUDE_CONFIG_DIR=$W/claude
export GIT_CONFIG_GLOBAL=$W/gitconfig GIT_CONFIG_NOSYSTEM=1
export FACTORY_STUB_DIR=$W/stub FACTORY_BIN=$BIN PATH=$SRC/tests/stubs:$PATH

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
}
commit_fixed() {
  git add -A
  GIT_AUTHOR_DATE=2026-09-30T12:00:00+00:00 GIT_COMMITTER_DATE=2026-09-30T12:00:00+00:00 \
    git commit -qm "$1"
}
# The JSON key set of a board: every path, array indexes collapsed.
keys() { jq -S '[paths | map(if type == "number" then "[]" else . end) | join(".")] | unique'; }

# FR-16 on one v2 repo: dry run, migrate, check, status before and after,
# retro.md and project.toml, the undo, the re-run, the retros table twice.
migrate_row() {
  local repo=$1 feature=$2
  cd "$repo"
  node .docs/bin/docs.mjs status "$feature" --json > ../before.json
  echo "--- status $feature --json before, by node .docs/bin/docs.mjs: $(wc -c < ../before.json) bytes"
  echo
  t $BIN docs migrate --dry
  t git status --porcelain
  t $BIN docs migrate
  t $BIN docs check
  t $BIN docs status
  $BIN docs status "$feature" --json > ../after.json
  echo "--- key sets of status $feature --json, node before vs binary after"
  keys < ../before.json > ../before.keys; keys < ../after.json > ../after.keys
  t diff ../before.keys ../after.keys
  echo "--- the only key gone is retro's contents: v3 prints retro: null (board.rs:431),"
  echo "--- its numbers are the retros row below (FR-19). Every other value, node vs binary:"
  jq -S 'del(.retro)' ../before.json > ../before.v; jq -S 'del(.retro)' ../after.json > ../after.v
  t jq -c .retro ../after.json
  t diff ../before.v ../after.v
  echo "--- log.flagged and log.decide hold ids ascending after migration (AD-702); the same values with both sorted:"
  jq -S '.log.flagged |= sort | .log.decide |= sort' ../before.v > ../before.s
  jq -S '.log.flagged |= sort | .log.decide |= sort' ../after.v > ../after.s
  t diff ../before.s ../after.s
  t git status --porcelain -- 'docs/features/*/retro.md' 'docs/archive/*/retro.md'
  t cat docs/project.toml
  sqlite3 -header "$FACTORY_DATA_DIR/factory.db" 'SELECT * FROM retros ORDER BY repo, feature' > ../retros-1.txt
  echo '$ sqlite3 factory.db "SELECT * FROM retros" (first run)'
  cat ../retros-1.txt; echo
  rm -rf ../first && cp -r docs ../first
  echo "--- the undo"
  t git checkout -- docs
  t git clean -fdq docs
  t git status --porcelain
  t node .docs/bin/docs.mjs check
  echo "--- the re-run"
  t $BIN docs migrate
  t diff -r ../first docs
  sqlite3 -header "$FACTORY_DATA_DIR/factory.db" 'SELECT * FROM retros ORDER BY repo, feature' > ../retros-2.txt
  echo '$ sqlite3 factory.db "SELECT * FROM retros" (after the re-run)'
  cat ../retros-2.txt; echo
  echo "--- the two reads: only written_at may differ"
  t diff ../retros-1.txt ../retros-2.txt
  cd - >/dev/null
}

# ---------- FR-16: the checked-in v2 fixture ----------
V2=$W/fr16-fixture/repo
mkdir -p "$(dirname "$V2")"; newrepo "$V2"
cp -r "$SRC/tests/fixture/v2/.docs" "$SRC/tests/fixture/v2/docs" "$SRC/tests/fixture/v2/gate.sh" "$V2/"
(cd "$V2" && commit_fixed 'chore: v2 fixture repo')

# ---------- FR-16: this repo's docs ----------
# HEAD still holds the slice fragments the driver merged for this checkpoint
# (migrate refuses an unmerged fragment, AD-706), so the copy is `git archive
# HEAD docs` with the working tree's merged 002 folder laid over it: the docs
# the checkpoint commit will hold.
OWN=$W/fr16-own/repo
mkdir -p "$(dirname "$OWN")"; newrepo "$OWN"
git -C "$SRC" archive HEAD docs | tar -x -C "$OWN"
rm -rf "$OWN/docs/features/002-docs-port"
(cd "$SRC" && git ls-files docs/features/002-docs-port | while read -r p; do
  [ -e "$p" ] && case "$p" in */evidence/cp-2-*) ;; *) cp --parents "$p" "$OWN/";; esac
done)
cp -r "$SRC/tests/fixture/v2/.docs" "$OWN/"
(cd "$OWN" && commit_fixed 'chore: this repo docs at the checkpoint')

{
  echo "# CP-2 FR-16 — branch binary $($BIN --version), $(date -I)"
  echo
  echo "## The v2 fixture (tests/fixture/v2)"
  echo
  FACTORY_DATA_DIR=$W/data16a migrate_row "$V2" 001
  echo "## This repo's docs (git archive HEAD docs + the merged 002 folder)"
  echo
  FACTORY_DATA_DIR=$W/data16b migrate_row "$OWN" 001
  echo "## The PRD-001 copy test"
  echo
  cd "$SRC"
  t cargo test -q --test migrate -- --ignored
} 2>&1 | f > "$EV/cp-2-fr16.txt"

# ---------- FR-17 routing half and FR-18's v2 refusal: v2 repos ----------
SD=$W/fr17/repo
mkdir -p "$(dirname "$SD")"; newrepo "$SD"
cp -r "$SRC/tests/fixture/v2/.docs" "$SRC/tests/fixture/v2/docs" "$SD/"
cp "$SRC/tests/fixture/stub-docs/docs.mjs" "$SD/.docs/bin/docs.mjs"
(cd "$SD" && commit_fixed 'chore: v2 repo with the stub docs.mjs')
{
  echo "# CP-2 FR-17 (routing half) — branch binary $($BIN --version), $(date -I)"
  echo
  echo "v2 repo whose .docs/bin/docs.mjs is tests/fixture/stub-docs/docs.mjs (echoes args, exits 3)."
  echo
  cd "$SD"
  t $BIN docs status 001 --json
  t $BIN docs check
  echo "The hook body (archive edit, broken link) and the skills grep are slice 11's: not built at CP-2."
} 2>&1 | f > "$EV/cp-2-fr17.txt"

{
  echo "# CP-2 FR-18 — branch binary $($BIN --version), $(date -I)"
  echo
  cd "$SRC"
  echo "## node spawns in src"
  t grep -rn 'Command::new("node")' src
  echo "## factory run on the v2 fixture"
  cd "$V2"
  git checkout -q -- docs; git clean -fdq docs
  t git status --porcelain
  FACTORY_DATA_DIR=$W/data18 t $BIN run 001
  t ls "$W/data18"
  echo "## contract asset grep (slice 10 task 9)"
  cd "$SRC"
  t grep -n 'agent-log\|log-\*\.md\|fragment\|ideas\.md\|render' assets/autonomy.md assets/pr-body.md assets/contract-*.md
  echo "## cargo test on the v3 fixture: the checkpoint gate run (gates/2-2.log)"
  echo "\$ grep 'test result' $GATELOG | awk '{p+=\$4; f+=\$6; i+=\$8} END {...}'"
  grep 'test result' "$GATELOG" | awk '{p+=$4; f+=$6; i+=$8; n++} END {printf "%d suites: %d passed; %d failed; %d ignored\n", n, p, f, i}'
  echo
  echo "\$ grep -n 'contract' $GATELOG"
  grep -n 'contract' "$GATELOG"
  echo
  echo "## the slice-contract command test on its own"
  t cargo test -q --test closeout the_slice_contracts_commands_run_on_the_fixture
} 2>&1 | f > "$EV/cp-2-fr18.txt"

# ---------- FR-19 closeout half: a whole stub run to ship ----------
V3=$W/fr19/repo
mkdir -p "$(dirname "$V3")"
git init -q --bare -b main "$W/fr19/origin.git"
newrepo "$V3"
git -C "$V3" remote add origin "$W/fr19/origin.git"
cp "$SRC/tests/fixture/gate.sh" "$V3/gate.sh"; chmod 755 "$V3/gate.sh"
cd "$V3"
$BIN docs init >/dev/null
printf '[docs]\nframework = 3\ngate = "./gate.sh"\ninner = "./gate.sh --inner"\nlanes = 2\ngate_parallel = true\n' > docs/project.toml
$BIN docs new fixture --title Fixture >/dev/null
for s in a b; do $BIN docs slice 001 $s >/dev/null; done
$BIN docs slice 001 c --depends 2 >/dev/null
python3 - docs/features/001-fixture/prd.md <<'PY'
import sys
p = sys.argv[1]
out, skip = [], False
for line in open(p).read().splitlines():
    if skip and line.startswith("  "):
        continue
    skip = False
    if line == "summary: |":
        out += [line, "  Fixture PRD, approved for the harness run.", "  Three slices; c waits for b."]
        skip = True
    elif line.startswith("**Harness**:"):
        out.append("**Harness**: none — fixture, gate-only")
    elif line.startswith("| FR-1 |"):
        for n, name in (("01", "a"), ("02", "b"), ("03", "c")):
            out.append(f"| FR-1 | run slice {n} ({name}) in the fixture → the board shows it done | test transcript | no |")
    else:
        out.append(line)
open(p, "w").write("\n".join(out) + "\n")
PY
$BIN docs set 001 status=ready >/dev/null
$BIN docs set 001 slices.2.depends_on=1 >/dev/null
mkdir -p src scenario; echo 'fixture code file' > src/lib.txt
printf '/scenario/\n' > .gitignore
git add -A && git commit -qm 'chore: fixture repo'
printf '%s\n' '{"outcome":"done"}' '{"outcome":"done"}' '{"outcome":"checkpoint"}' \
  '{"outcome":"done"}' '{"outcome":"checkpoint"}' '{"outcome":"polish"}' > "$W/stub/plan.jsonl"
{
  echo "# CP-2 FR-19 (closeout half) — branch binary $($BIN --version), $(date -I)"
  echo
  echo "v3 repo built like Fixture::new + chain(), stub claude and stub gh on PATH, plan: done, done, checkpoint, done, checkpoint, polish."
  echo
  t $BIN run 001
  RUN=$(ls "$W/data/runs")
  for _ in $(seq 1 180); do
    [ "$(jq -r .phase "$W/data/runs/$RUN/state.json" 2>/dev/null)" = finished ] && break
    sleep 1
  done
  t jq -r .phase "$W/data/runs/$RUN/state.json"
  t git log --format=%s
  t $BIN docs status 001
  echo "## retro.md has no frontmatter: its first lines"
  t head -3 docs/features/001-fixture/retro.md
  echo "## the retros row"
  t sqlite3 -line "$W/data/factory.db" "SELECT * FROM retros"
  echo "run id of this run: $RUN"
  echo
  echo "## the ## Numbers section of the same run's retro.md"
  t sed -n '/^## Numbers/,$p' docs/features/001-fixture/retro.md
  echo "## factory report for the same run (the row's source)"
  t $BIN report "$RUN"
  echo "## factory retro before and after --set score=4"
  t $BIN retro "$RUN"
  t $BIN docs retro 001 --set score=4
  t $BIN retro "$RUN"
  t sqlite3 "$W/data/factory.db" "SELECT run_id, score FROM retros"
  t git status --porcelain
} 2>&1 | f > "$EV/cp-2-fr19.txt"

echo "work dir: $W"
