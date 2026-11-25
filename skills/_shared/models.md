# Model routing for this skill family

`/prd`, `/slice`, `/polish`, `/harness`, `/ux`, `/gate` and `/deliver` share this file. It exists so a skill can say **which kind of model a job needs** without naming one. Model names change every few months; roles do not.

**Read this only when you are about to delegate.** The common case — a skill running in the main thread — needs nothing from here. Each skill states its own bar in its own body, in prose, so the cheap path stays free.

## The four roles

| Role | For | Default |
|---|---|---|
| **judgment** | interviews, triage, adversarial passes, anything where being wrong is expensive | the strongest reasoning model available (`opus`, or `fable` when it is the session model) |
| **mechanical** | bulk reading, per-file sweeps, scans whose answer is a list | the fast model (`haiku` for scans, `sonnet` when the output needs shaping) |
| **vision** | anything that looks at a screenshot | any vision-capable model; today all of the above qualify |
| **second opinion** | a reviewer that did not write the thing it is reviewing | a fresh agent with no prior context, **always cross-vendor** — see below |

In `/deliver`, a slice builder is **judgment** — it makes every call the slice left open, alone — and the subagent that reads a failed CI log is **mechanical**.

Pass the role's model as the model argument when your harness starts a subagent (in Claude Code, `model:` on the Agent tool, accepting `haiku`, `sonnet`, `opus`, `fable`); in another harness, its own tier with the same role. **Nowhere else in this family names a model.** If a value here is rejected, use the closest available tier, say so once, and fix this file — never block the work on it.

## Overrides

Edit the table above. A role you never touch keeps its default. There is no setup command and no interview: four roles across seven skills does not earn one, and a config file nobody can read by hand is worse than a default nobody changed.

## Second opinion, specifically

The value is **fresh eyes**, and cross-vendor eyes are better than fresh ones from the same family. Opus, Sonnet and Fable share training and share blind spots, so a second agent from the same vendor agreeing with the first is weak evidence. A different vendor disagreeing is strong evidence.

**`factory review second` is the cross-vendor path**, using `[review]` in `~/.config/factory/config.toml` (configured via `factory setup`): the owner picks the reviewer's provider and model there, once, for every repo. Do not probe for another vendor's CLI or plugin, and do not branch on one being present. A tool being installed says nothing about whether its subscription is live, so presence is not a signal. Choose a provider from a different vendor than the one writing the code: cross-vendor is the point.

**Run it through `factory review second`**: `--diff <base> [-- <paths>]` for a diff, `--doc <file> --prompt "<text>"` for a document or a question. The command carries the flags, the pass size and the reading of the stream; `[review]` holds the provider and the model. Exit 2 is `disabled:` (the owner opted out) or `unreviewed:` (the reviewer could not run); its last line says which.

Second opinion unavailable or unreviewed → a fresh subagent on the `judgment` model with **no inherited context**. An explicitly disabled second opinion (`disabled:`) skips the fallback: the owner chose none, so say the second opinion was disabled and go on. Say which reviewer you used and which model. Never a fork here (`subagent_type: "fork"` in Claude Code): a fork inherits the reasoning you are asking it to attack, which is the whole failure being avoided.

## Stating the bar — the preflight rule

A skill that does better on a stronger model says so **in one line, only when the session is under its bar**, then continues:

> `/prd` does its sharpest interview on a top-tier reasoning model. This session is on Haiku. Continuing — interrupt and `/model` if you want the stronger pass.

Rules that keep this from becoming noise:

- **Silent when the session is at or above the bar.** That is the common case and it must cost nothing.
- **Never a question, never blocking.** A skill cannot change the session model; only the user can. Telling them is the whole contribution.
- **Once per run**, at preflight, not repeated at each step.
- **No bar at all** for a skill where the model does not change the outcome. `/gate` is measurement and shell work; a note there would be pure noise.

## Preconditions are a different thing

A bar is a preference. A **precondition** is a capability the job cannot proceed without, and it is handled by degrading in a stated way, not by a note.

`/ux` is the only one in this family: without vision it cannot judge a screenshot. It still runs every check that comes from grep and the accessibility tree, and it reports which half it did and which it could not. Same posture as a missing harness — say what went unreviewed, never imply coverage you do not have.
