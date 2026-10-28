---
name: trust-agent-orchestration
description: Runs truST implementation work through cheap subagents (opencode with DeepSeek V4.1 Flash Free) while the lead plans, briefs, verifies and commits. Use when splitting a slice into agent tasks, writing a brief, launching or following up an agent, reviewing an agent's work, or when an agent run hangs, ends early or reports success without evidence.
---

# truST agent orchestration

The owner's decision of 17 October 2026: implementation subagents run in opencode on DeepSeek V4.1
Flash, and the lead (Claude) orchestrates. Claude's tokens go to planning, briefs, review and
commits.

Since 18 October the agents run the free model `opencode/deepseek-v4.1-flash-free` (the owner's
choice). Its rate cap stopped six parallel agents within minutes on 17 October; when it stops
runs again, launch with `OC_MODEL=opencode-go/deepseek-v4.1-flash`, the paid model of the
owner's OpenCode Go plan (a few cents per task), and tell the owner. Every report and
`oc-status.sh` show the cost.

The tooling lives outside the repository, in `~/projects/.artifacts/`. Only the rules are here.

## The tools

| File | Purpose |
|---|---|
| `oc-agent.sh <worktree> <task-id> <brief>` | Runs one agent, in the background (`run_in_background`) |
| `oc-status.sh [task...]` | One line per task: running or done, tool calls, last tool. It reads the events files, which fill only when a run ends |
| `oc-stop.sh <title>` | Stops an agent: interrupts its busy sessions on the server |
| `oc-live.py [title...]` | Live progress of running agents from opencode's database (read-only; truST worktrees only): messages, last activity, cost |
| `oc-tasks/<task>.md` | The briefs |
| `oc-runs/<task>/` | Per call N: `report-N.md` (the only file the lead reads in full), `events-N.jsonl`, `diffstat-N.txt` |
| `opencode/trust-agents.json` | The model, and permission denials: builds, git writes, ssh, devices, kill, sudo |
| `opencode/agent-rules.md`, `opencode/coding-guidelines.md`, `agent-pitfalls.md` | Prepended to every call |
| `opencode/brief-template.md` | The brief's shape, with the research behind it |
| `agent-check.sh` | The agents' compile check (`trust-remote-builder`). Its chip lanes deny warnings, as CI's portable lane does; `CHIPS=fw-esp32c6` (or `fw-stm32f407`) also builds that firmware and prints its link errors and RAM sections |
| `agent-fixtures.sh <wt> <crate> <test> <TRUST_WRITE_..._FIXTURES>` | Regenerates a crate's golden fixtures on the builder and copies them back; agents cannot use ssh themselves |

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
   completion notification, and use `oc-live.py` when the owner asks.
9. **One agent per crate per worktree at a time.** Two agents may share a worktree only when they
   change different crates and neither crate depends on the other (`trust-devices` depends on
   `trust-runtime`, so they count as one). Otherwise one agent's half-written code fails the other's compile
   check, and the snapshot diff of a call also shows the other agent's changes. On 17 October C2a's
   unfinished `StatusPort` changes failed C3a's checks. Put parallel work on the same crate in
   separate worktrees, or run it one after the other. The lead commits.

10. **A brief that touches registered code says so.** When a task moves lines in a file with
    registered `unsafe` sites, or adds an `unsafe` site or a concurrency boundary (atomics,
    mutexes, statics), the brief lists `xtask/config/full_map_policy.json` among the allowed files.
    The agent's compile check runs both register gates. Firmware crates outside the workspace have
    their own `Cargo.lock`: when an adapter gains a dependency, the lead updates that lock on the
    builder (`cargo metadata --manifest-path firmware/<fw>/Cargo.toml`), pinning shared crates to the
    root lock's versions.

11. **Read your brief against itself before sending it.** A requirement must not need a file or
    a change that the "Files you may change" or "Do not" lists forbid. On 17 October, C5-2b's
    brief asked for STRN payload bytes but forbade touching the codec, and it left out the two
    files the per-scan snapshot runs through. Trace each requirement's code path, and list every
    file on it.
12. **Every focused check runs the gates:** the external unsafe register, both runtime gates and the
    architecture doctor (with the git stand-in). They take seconds. On 17 October C5-2a's check
    skipped them, and its committed codec move failed `RUNTIMESAFE-RETAIN-NO-CHECKSUM`, because the
    gate still looked in the codec's old place.

13. **A run without a session is stuck.** `opencode run` reads its stdin to the end before it sends
    the message, so the wrapper gives it `< /dev/null`; on 17 October seven runs started from
    background shells hung for up to 90 minutes without it. An empty events file proves nothing
    while a run is going (the file fills at the end). Check with `oc-live.py`: a run that has no
    session a minute after its start is stuck. Kill its `opencode run` by PID, move the call's
    files to `hung/`, and launch it again.

14. **Stop an agent on the server, not by killing its client.** An `opencode run` session runs on the
    server; killing the client leaves it editing the worktree (18 October: a stray C1-3c2a session
    worked beside its relaunch for 30 minutes). Use `oc-stop.sh <title>`, which interrupts every
    busy session of that title through `POST /api/session/<id>/interrupt`. Launch agents only with
    Bash `run_in_background`, never with a plain `&`.

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
