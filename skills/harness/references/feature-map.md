# Feature map — the shape

`.claude/skills/verify-<app>/features/README.md` is the index; one file per user-facing feature sits beside it. The map is the repo's maintained verification source: a Prove task cites a feature file, and `/harness maintain` audits every file against the running app.

**Keep implementation details out.** Only user paths, stable handles, required state, exact commands, and observable proof. A controller name helps nobody who is driving the app.

The example below is illustrative. Generated files carry the repo's real routes, labels and helpers — nothing is invented.

## README.md

```markdown
# {App} verification map

Read this index before driving the app, then use the matching feature file as the recipe.

## Baseline preconditions

- {App launched per the skill's Launch section; URL / binary.}
- {Isolation: the env var, port or DB name per run.}
- {Seed: `php artisan db:seed --class=RulesQaSeeder` — what it creates.}
- {Doctor passes and reports: what it must say.}
- Never drive an instance this run did not start.

## Driving conventions

- Start every recipe from the baseline state unless its preconditions say otherwise.
- Scripted drives: {Pest browser in `tests/Browser`}. Interactive: `terminal-browser action -- …`.
- Prefer ARIA roles and accessible names over CSS selectors or DOM position.
- Treat every command as literal. Keep quoted names and flags unchanged.
- Restore seeded data after a mutation. Never remove proof artifacts during cleanup.

## Proof and skip reporting

- Capture the user action and the resulting state, not only the final screen.
- UI proof = ARIA snapshot + screenshot with the app identity visible. CLI proof = command, stdout, stderr, exit code. Mutation proof = a read-only second view of the stored value.
- Record the feature id and the entry point used with every artifact.
- Report an unreachable path with the attempted command and the unmet precondition. Never report a skipped entry point as verified through a different path.

## Feature entry contract

Each feature file: an H1, one paragraph on the user-visible behavior, then exactly four H2s in this order.

1. `Sub-features` — short ids, one line each.
2. `How to get to it (user POV)` — every entry point.
3. `Driving it with {driver}` — starts with `Preconditions:`, then labelled bullets pairing each user action with an exact command and the observable result.
4. `Gotchas` — traps that waste or invalidate a run.

## Features

- [{Feature}](./{feature}.md) — {one line: what it covers}.
```

## One feature file

```markdown
# Create a rule

Create rule lets an admin add a proactive rule to an agent from the Playbook tab, see it in the rules list, and have it fire on the widget when its conditions match.

## Sub-features

- `rule-open` opens the rule form from the Playbook tab.
- `rule-save` persists name, conditions and action.
- `rule-list` shows the saved rule with its status.
- `rule-fires` the widget opens with the rule's message on a matching page.

## How to get to it (user POV)

- Agents → {Agent} → **Playbook** tab → `New rule`.
- Direct route: `/agents/{id}/edit/playbook`.

## Driving it with Pest browser

Preconditions:

- `RuleEngineQaSeeder` has run; the QA agent exists with zero rules.
- Doctor reports the test server up and the seeded agent key.

- **Open form.** Choose `New rule`. `visit("/agents/{$agent->id}/edit/playbook")->click('New rule')`. A form named `Rule` appears with focus in `Name`.
- **Fill and save.** `->fill('Name', 'Pricing nudge')->select('Condition', 'URL contains')->fill('Value', '/pricing')->click('Save rule')`. A status `Rule saved` appears and the list shows `Pricing nudge` as `active`.
- **Read back.** `->navigate("/agents/{$agent->id}/edit/playbook")->assertSee('Pricing nudge')`. The row persists across a reload.
- **Fires on the widget.** `config(['testbed.agent_key' => $agent->agent_key])`, then `visit('/testbed/pricing')` and wait for `.ca-panel[data-open="true"]` — see the `waitForFunction` helper in `tests/Browser/ProactiveOpenTest.php`. The panel shows the rule's message.
- **Interactive equivalent.** `terminal-browser open http://127.0.0.1:8000/agents/{id}/edit/playbook --split right`, then `action -- snapshot`, `action -- click @<New rule>`, `action -- fill @<Name> "Pricing nudge"`, `action -- screenshot artifacts/<run>/rule-save.png`.
- **Proof.** `->screenshot('rules-list')` after read-back, plus the widget screenshot. Both show the app header and `Pricing nudge`.

## Gotchas

- Saving with an empty condition value is allowed and yields a rule that never fires — assert the condition row, not only the name.
- The widget testbed reads the agent key from `config('testbed.agent_key')`; set it before `visit()`.
- Rules are `once_per_visitor`: a second fire needs a fresh browser context or a cleared dossier.
- Remove `Pricing nudge` in cleanup; keep its artifacts.
```
