# QA Guide — PRD-016 Factory as a consumer

**Time budget:** ~5 minutes (after merging the PR into `main`)
**Scope:** Only the 1 Human row from [`prd.md`](prd.md) §8 QA plan — live GitHub Actions execution of `.github/workflows/edge.yml` on `main`, which cannot run before merge.

## 1. Live rolling `edge` prerelease workflow (highest priority — FR-1)

1. After merging `feat/factory-as-a-consumer` into `main`, open the Actions tab (`gh run list --workflow edge.yml --limit 3`) and confirm `.github/workflows/edge.yml` triggered automatically on the `main` push.
2. Wait for the workflow run to complete green across all 4 Unix targets and `publish`.
3. Verify the published prerelease on GitHub Releases (`gh release list --limit 5`):
   - Tag follows `vX.Y.Z-edge.<run_number>` (sorting strictly above the latest tagged stable release) and is marked **Pre-release**.
   - Assets include `factory-installer.sh` and all 4 target archives.
4. Run `factory upgrade --check --edge` locally and confirm it sees the new edge prerelease, while plain `factory upgrade --check` ignores it.
5. (Optional) Trigger a second `workflow_dispatch` run on `edge.yml` and confirm the older `*-edge.*` prerelease and tag are pruned so only the newest rolling edge prerelease remains.
