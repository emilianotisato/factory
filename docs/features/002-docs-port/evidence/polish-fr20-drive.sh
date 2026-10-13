#!/usr/bin/env bash
# PRD-002 polish: drives the FR-20 rows no checkpoint drove (slice 05 task 8,
# slice 08 task 7) on the branch binary, one transcript.
set -u
SRC=/home/emiliano/Projects/factory
BIN=${BIN:-/mnt/devcache/emiliano/cargo-target/debug/factory}
mkdir -p /tmp/polish-fr20
W=$(mktemp -d /tmp/polish-fr20/work.XXXX)
: > "$W/gitconfig"
export GIT_CONFIG_GLOBAL=$W/gitconfig GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
export FACTORY_DATA_DIR=$W/data FACTORY_CONFIG_DIR=$W/config
mkdir -p "$W/data" "$W/config"

t() {
  echo "\$ $*"
  "$@" 2>&1
  echo "[exit $?]"
  echo
}

echo "# FR-20 — migration half: the v2 fixture's ideas.md"
echo
M=$W/migrate
cp -r "$SRC/tests/fixture/v2" "$M"
cd "$M" && git init -q && git add -A && git commit -qm fixture
t "$BIN" docs migrate --dry
t git status --porcelain
t "$BIN" docs migrate
t ls docs/ideas
for f in docs/ideas/*.md; do
  [ "$(basename "$f")" = README.md ] && continue
  echo "--- $f"
  cat "$f"
  echo
done
t "$BIN" docs idea list --json
t "$BIN" docs check

echo "# FR-20 — tool half, in a fresh v3 repo"
echo
V=$W/v3
mkdir -p "$V" && cd "$V" && git init -q
t "$BIN" docs init
git add -A && git commit -qm init
echo "## concurrent idea add, same title, 2 x 10"
for p in 1 2; do
  ( for i in $(seq 10); do printf 'Body\n' | "$BIN" docs idea add --title Same --body-file - >/dev/null; done ) &
done
wait
t sh -c 'ls docs/ideas | grep -c "same"'
ID=$(printf 'Will go.\n' | "$BIN" docs idea add --title Doomed --body-file -)
git add -A && git commit -qm ideas
echo "## idea drop"
t "$BIN" docs idea drop "$ID" --because "polish drive"
t git show --name-status --format=%B HEAD
echo "## new --from-idea"
FROM=$(printf 'Carried body.\n' | "$BIN" docs idea add --title Seed --body-file -)
t "$BIN" docs new demo --from-idea "$FROM"
t sh -c "ls docs/ideas/$FROM.md 2>&1; grep -n 'from_idea\|## From idea\|Carried body' docs/features/*-demo/prd.md"
t git status --short
echo "## check on a bad idea file"
printf 'no frontmatter\n' > docs/ideas/2026-10-13-broken.md
t "$BIN" docs check
rm -rf "$W"
