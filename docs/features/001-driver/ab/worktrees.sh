#!/usr/bin/env bash
# FR-11 step 2: the two A/B worktrees, cut from one commit and set up alike.
#   worktrees.sh <repo> <base-ref> <NNN> <slug>
# Run A (/deliver) gets feat/NNN-slug in ../wt-a, run B (factory) gets
# feat/slug in ../wt-b. factory accepts only the plain name, /deliver accepts
# both, and git refuses one branch in two worktrees.
set -euo pipefail
[ $# -eq 4 ] || { echo "usage: $0 <repo> <base-ref> <NNN> <slug>" >&2; exit 2; }
repo=$(realpath "$1") base=$2 nnn=$3 slug=$4
parent=$(dirname "$repo")
sha=$(git -C "$repo" rev-parse --verify "$base^{commit}")
git -C "$repo" worktree add -q -b "feat/$nnn-$slug" "$parent/wt-a" "$sha"
git -C "$repo" worktree add -q -b "feat/$slug" "$parent/wt-b" "$sha"

for wt in "$parent/wt-a" "$parent/wt-b"; do
  # Gitignored files the gate, the harness or the agent's tools read.
  for f in .env .env.testing .mcp.json boost.json; do
    if [ -e "$repo/$f" ]; then cp -a "$repo/$f" "$wt/$f"; fi
  done
  # Generated project skills (Laravel Boost) are gitignored; tracked ones are
  # already in the worktree.
  for d in "$repo"/.claude/skills/*/; do
    name=$(basename "$d")
    if [ -z "$(git -C "$repo" ls-files ".claude/skills/$name")" ]; then
      cp -a "$d" "$wt/.claude/skills/$name"
    fi
  done
  (
    cd "$wt"
    if [ -f composer.json ]; then composer install -q --no-interaction; fi
    if [ -f package-lock.json ]; then npm ci --silent --no-audit --no-fund; fi
    if [ -f package.json ] && grep -q '"build"' package.json; then npm run build --silent >/dev/null; fi
    if [ -f go.mod ]; then go mod download; fi
  )
  echo "ready: $wt on $(git -C "$wt" branch --show-current) at ${sha:0:7}"
done
