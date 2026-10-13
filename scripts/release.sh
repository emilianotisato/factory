#!/bin/sh
# Cut a release: tag the version Cargo.toml names, on a clean and current main,
# and push the tag. The push starts .github/workflows/release.yml (cargo-dist).
#
# The bump itself happens in the PR: the bundle.lock gate test refuses a skill
# edit without one and says how to relock. This script only refuses the mistakes
# a tag can make: wrong branch, dirty tree, behind origin, a lock that does not
# match Cargo.toml, a tag that already exists.
set -eu

cd "$(git rev-parse --show-toplevel)"

refuse() {
  echo "release: $1" >&2
  exit 1
}

branch=$(git symbolic-ref --short HEAD 2>/dev/null || echo detached)
[ "$branch" = main ] || refuse "on $branch, not main"
[ -z "$(git status --porcelain)" ] || refuse "working tree is not clean"
git fetch -q origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || refuse "main is not at origin/main; pull or push first"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
[ -n "$version" ] || refuse "no version in Cargo.toml"
lock=$(sed -n 's/^version = "\(.*\)"/\1/p' skills/bundle.lock)
[ "$lock" = "$version" ] || refuse "skills/bundle.lock says $lock, Cargo.toml says $version; FACTORY_BUNDLE_WRITE=1 cargo test, then commit"

tag="v$version"
git rev-parse -q --verify "refs/tags/$tag" >/dev/null && refuse "$tag already exists; bump Cargo.toml first"
git ls-remote --exit-code --tags origin "refs/tags/$tag" >/dev/null 2>&1 && refuse "$tag already exists on origin; bump Cargo.toml first"

git tag -a "$tag" -m "factory $version"
git push origin "$tag"
echo "pushed $tag: https://github.com/emilianotisato/factory/actions/workflows/release.yml"
