#!/usr/bin/env sh
# The fixture's gate. Its outcome is scripted by the test, not by the stub
# agent: without --inner it consumes the first line of scenario/gate, with
# --inner the first line of scenario/inner. `red` exits 1 with a fixed failure
# text; anything else, an empty file or no file is green. Every run appends
# `<epoch> <inner|gate> <exit> <pwd> <head>` to scenario/gate.log: the
# directory it ran in (`pwd -P`) and the HEAD it saw (`-` when unborn).
# scenario/ is gitignored, so a lane worktree has none: it is found next to the
# git common dir, the main tree's from every lane, and next to this script when
# git finds no repo.
set -u
here=$(cd "$(dirname "$0")" && pwd)
if common=$(git -C "$here" rev-parse --path-format=absolute --git-common-dir 2>/dev/null); then
  scenario="$(dirname "$common")/scenario"
else
  scenario="$here/scenario"
fi
kind=gate
if [ "${1:-}" = "--inner" ]; then kind=inner; fi
file="$scenario/$kind"
outcome=green
if [ -s "$file" ]; then
  outcome=$(head -n 1 "$file")
  tail -n +2 "$file" > "$file.tmp" && mv "$file.tmp" "$file"
fi
code=0
if [ "$outcome" = "red" ]; then
  code=1
  echo "FAIL tests/fake.rs: expected 2 got 3"
fi
head=$(git rev-parse --verify -q HEAD 2>/dev/null) || head=-
mkdir -p "$scenario"
printf '%s %s %s %s %s\n' "$(date +%s)" "$kind" "$code" "$(pwd -P)" "$head" >> "$scenario/gate.log"
exit $code
