---
name: trust-agent-orchestration
description: Runs truST implementation work through cheap subagents (opencode with DeepSeek V4.1 Flash Free) while the lead plans, briefs, verifies and commits. Use when splitting a slice into agent tasks, writing a brief, launching or following up an agent, reviewing an agent's work, or when an agent run hangs, ends early or reports success without evidence.
---

# truST agent orchestration

The owner's decision of 17 October 2026: implementation subagents run in opencode on DeepSeek V4.1
Flash, and the lead (Claude) orchestrates. Claude's tokens go to planning, briefs, review and
commits.

The free tier's cap stopped six parallel agents within minutes. The agents therefore run
`opencode-go/deepseek-v4.1-flash` through the owner's OpenCode Go plan, and its pay-as-you-go
balance once the plan's limit is used. That is a few cents per task. Every report and
`oc-status.sh` show the cost. Stay within what the work needs, and tell the owner the day's total.

The tooling lives outside the repository, in `~/projects/.artifacts/`. Only the rules are here.

## The tools

| File | Purpose |
|---|---|
| `oc-agent.sh <worktree> <task-id> <brief>` | Runs one agent, in the background (`run_in_background`) |
| `oc-status.sh [task...]` | One line per task: running or done, tool calls, last tool |
| `oc-tasks/<task>.md` | The briefs |
| `oc-runs/<task>/` | Per call N: `report-N.md` (the only file the lead reads in full), `events-N.jsonl`, `diffstat-N.txt` |
| `opencode/trust-agents.json` | The model, and permission denials: builds, git writes, ssh, devices, kill, sudo |
| `opencode/agent-rules.md`, `opencode/coding-guidelines.md`, `agent-pitfalls.md` | Prepended to every call |
| `opencode/brief-template.md` | The brief's shape, with the research behind it |
| `agent-check.sh` | The agents' compile check (`trust-remote-builder`) |

## Rules for the lead

1. **Brief by the template.** Every brief has:
   - the exact files to read, with line ranges;
   - the only files the agent may change;
   - numbered requirements, each with a named test and the way to reach its code path;
   - ordered steps, and the exact compile check command;
   - a rule for gaps (keep today's behaviour, report the gap);
   - how the work is judged.

   At most about 600 changed lines per task: split larger slices. A loose brief is the most likely
   cause of a missed requirement.
2. **Verify; never trust a claim.** Check, in this order:
   - scope: `diffstat-N.txt` against the allowed files;
   - the real compile check output in `events-N.jsonl`, or in
     `~/.local/share/opencode/shell/<hash>/sh_*.out` when the check ran in the background;
   - the tests against `coding-guidelines.md`: expected values from the specification, and each
     test naming the bug it catches;
   - the diff.

   Then run the focused tests in the wave's batch.
3. **Follow-ups are fresh sessions.** Never continue an opencode session (`--session`): it hangs on
   our server. The wrapper gives a follow-up the original brief and the last report. If a call
   failed or hung, move its empty `events-N` and `report-N` aside (`oc-runs/<task>/hung/`) first.
4. **Escalate after two failed rounds**, or when edits drift outside the allowed files. Hand the
   task to a Claude agent or do it yourself.
5. **Use the agents' own server** (`127.0.0.1:4097`, started by the wrapper). Never use the owner's
   opencode service, which runs the owner's own sessions. `--standalone` boots in about 105 s on
   the Pi.
6. **Never edit a running script in place.** Bash reads a script as it runs, so write
   `<script>.new` and `mv` it over the old one.
7. **Never print credential files.** opencode's `service.json` and the server log contain
   passwords. Filter them, or extract only the field you need into a file with mode 600.
8. **Do not poll agents with a progress monitor.** Each event costs a lead turn. Rely on the run's
   completion notification, and use `oc-status.sh` when the owner asks.
9. **One agent per crate per worktree at a time.** Two agents may share a worktree only when they
   change different crates. Otherwise one agent's half-written code fails the other's compile
   check, and the snapshot diff of a call also shows the other agent's changes. On 17 October C2a's
   unfinished `StatusPort` changes failed C3a's checks. Put parallel work on the same crate in
   separate worktrees, or run it one after the other. The lead commits.

## Calibration (17 October 2026)

- The STRING character-write cut had a loose brief. The code compiled, but it tested one of three
  store paths and wrote no report, because it ended its turn while its check ran in the background.
- C3a, the search sources, was briefed with the design's exact test names. Good result: the
  specification first, deterministic fakes, an honest report, and one file over the 1,000-line
  limit.
- C2a, the status fields, was the first brief written by the template. Every requirement was met
  with its named test, it changed exactly the allowed files, and its report was complete and
  honest. Its one foreground compile check was all ok. It cost 248k free tokens and about 10k lead
  tokens to review. The one weak spot: an expected value was taken from the fake under test. The
  coding guidelines now ask for literal expected values.
- The research (SiliconFlow, AgentLens arXiv 2607.06624, PromptZone, MindStudio) agrees: Flash does
  well on clearly specified, automatically checked work. It relaxes scope and verification rules
  and sometimes overstates its own checks.
