#!/usr/bin/env sh
# A fixture gate that fails like a PHP project's: on `red` it prints a Pest
# JSON failure between noise lines and ends on composer's useless last line.
# Otherwise it reads scenario/ and writes scenario/gate.log as gate.sh does.
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
  echo "> composer ci:check"
  echo "Running 214 tests in 8 processes"
  echo '{"failures":[{"test":"it rejects an expired token","file":"tests/Feature/AuthTest.php","line":42,"message":"Failed asserting that 200 is 401."}]}'
  echo "Tests: 213 passed (604 assertions)"
  echo "Duration: 12.31s"
  echo "Script @ci:check was called via verify"
fi
head=$(git rev-parse --verify -q HEAD 2>/dev/null) || head=-
mkdir -p "$scenario"
printf '%s %s %s %s %s\n' "$(date +%s)" "$kind" "$code" "$(pwd -P)" "$head" >> "$scenario/gate.log"
exit $code
