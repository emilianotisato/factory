# PRD-013 — QA guide

From the QA plan's one Human row (prd §10, FR-4). **About 10 minutes**, after `factory cockpit install` and `/reload-plugins`, in a cockpit session during a live run.

1. **Most important: the status line reads at your real width (3 min).** In this repo, during a run, the status line shows `NNN · <phase> · n/m done · 12m` (`building`, `checkpoint` or `closeout`) and moves as slices land. Check it at your usual terminal width and at a narrow split.
2. **A BLOCK raises the band (3 min).** When the run blocks, one row appears above the prompt, starting with the feature number, and it goes away after `factory answer`.
3. **`/factory-board` opens and closes the pane (2 min).** The pane shows the board, and a second `/factory-board` closes it.
4. **Quiet where there is no run (2 min).** In a repo with no live run, the status line and the band stay empty, and `/factory-board` says `no live run in this repo`. A status line reading `factory: mod stale: …` instead means the mod is older than the binary: re-run `factory cockpit install`.

## Results (2026-10-30)

Run by the owner with the `feat/live-status` binary and `factory cockpit install`, against PRD-004's live run in `Prokey/sii-api`.

- Check 1, status line: **pass**.
- Check 2, band on a BLOCK: **pass**.
- Check 3, pane toggle: **pass**. A second `/factory-board` replied `factory board closed.`
- Check 4, quiet with no run: **pass**, per the owner, in this repo.
