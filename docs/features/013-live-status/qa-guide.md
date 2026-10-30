# PRD-013 — QA guide

From the QA plan's one Human row (prd §10, FR-4). **About 10 minutes**, after `factory cockpit install` and `/reload-plugins`, in a cockpit session during a live run.

1. **Most important: the status line reads at your real width (3 min).** In this repo, during a run, the status line shows `NNN · building · n/m done · 12m` and moves as slices land. Check it at your usual terminal width and at a narrow split.
2. **A BLOCK raises the band (3 min).** When the run blocks, one row appears above the prompt, starting with the feature number, and it goes away after `factory answer`.
3. **`/factory-board` opens and closes the pane (2 min).** The pane shows the board, and a second `/factory-board` closes it.
4. **Quiet where there is no run (2 min).** In a repo with no live run, nothing shows in the status line, the band or the pane.
