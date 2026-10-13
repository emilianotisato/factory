#!/usr/bin/env sh
# The fixture's gate (PRD-001 FR-10). Its outcome is scripted by the test, not by
# the stub agent: without --inner it consumes the first line of scenario/gate,
# with --inner the first line of scenario/inner. `red` exits 1 with a fixed
# failure text; anything else, an empty file or no file is green. Every run
# appends `<epoch> <inner|gate> <exit>` to scenario/gate.log.
set -u
here=$(cd "$(dirname "$0")" && pwd)
kind=gate
if [ "${1:-}" = "--inner" ]; then kind=inner; fi
file="$here/scenario/$kind"
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
mkdir -p "$here/scenario"
printf '%s %s %s\n' "$(date +%s)" "$kind" "$code" >> "$here/scenario/gate.log"
exit $code
