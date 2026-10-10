# Jev replays (2026-10-09)

Three scripts that replayed the owner's Claude Code transcripts through TypeSafe's Jev
(`jev-1.13.0`, `POST /v1/systemone`) to test where a calibrated decision model could sit in
`factory`. Results and arithmetic: `../ctx-jev-research.md`, section "Replays with the owner's key".

- `probe.py`: latency from this machine, a line pick on a public stdlib file, pack trimming on PRD-001.
- `blocks.py`: every `AskUserQuestion` block in the transcripts, Jev's pick vs the owner's real answer.
- `readfor.py`: whole-file reads that led to an edit; Jev's window selection vs the edited lines.

Run with `TYPESAFE_API_KEY` exported and `REPLAY_OUT` pointing at a scratch folder. The transcript
roots are hardcoded to the owner's two dogfood projects; change them before reuse. Transcript text
leaves the machine when these run: the owner allowed that on 2026-10-09 for these two projects.
