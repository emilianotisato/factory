---
title: "Provider switches leave Claude slugs on provider-less role tables"
state: "open"
added: "2026-11-25"
origin: "PRD-027 polish"
---

AD-3 takes a role table's model/effort when the role's config provider (`[workers.<role>] provider`, else `[workers] provider`, else the env) equals the resolved one. AD-6 reads a provider-less table as Claude's in `factory setup workers`. The two disagree once `[workers] provider` moves off Claude while a role table holds a model and no provider:

- `[workers.build] model = "sonnet"`, no provider; answering the worker ask `codex:gpt-5.4:xhigh`, or `factory setup workers --provider codex`, makes build `codex -m sonnet` (and a role `effort = "max"` reaches `-c model_reasoning_effort=max`, which is no Codex level).
- `FACTORY_WORKER_PROVIDER=codex` with `factory setup workers --effort max` on a provider-less table stores a Claude effort the Codex role then inherits.
- The `factory setup workers` wizard brackets the first CLI on PATH as the provider default, not the effective one (env or AD-6's Claude), so Enter can write `provider = "claude"` over an env choice or clear a legacy model; a second pass over the same role reads the pre-wizard file and can unset the model the first pass set.
- Model and effort read from config.toml are not checked against `is_model_name` or the provider's effort levels at load (`effort = "--x"` reaches agy argv).

Options: (a) when a provider change is written to `[workers]`, clear or pin (`provider = "claude"`) every provider-less role table that holds a model/effort; (b) make `resolve_role` read a provider-less role table as Claude's, overturning AD-3's fallthrough; (c) validate model/effort against the resolved provider in preflight and refuse. (a)+(c) keep AD-3 and fail loud.

Related label slip: `config::role_sources` computes `own` from the tables only, while `resolve_role` also counts `FACTORY_WORKER_PROVIDER`. With that env set to `claude`, `[workers.build] model = "sonnet"` and `--worker build=claude`, the run uses `sonnet` but `runs.knobs` labels the model `default` instead of `user`.
