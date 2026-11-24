### Added

- A computer runtime records cycle traces. `trace.record` starts a recording at the resource's next
  cycle, for a number of cycles, a task's runs or a span of logical time, with all of its memory
  reserved first. Each cycle keeps its task activations, the control link's writes as the bytes the
  variables then hold, its `CURRENT_DT` readings, the input image as the scan sampled it, the output
  image it wrote, and the fault that ended it. `trace.read` gives the recording's state and then its
  `.trusttrace` file in parts, each with its CRC-32, and `trace.stop` ends a recording at once. A
  recording ends early when its resource stops, pauses, restarts or faults, and when its program
  changes online (specification 48 RU-59; specification 11, "Cycle traces").
- `trust-runtime ctl record (--cycles <n> | --duration <t>) [--task <task>] [--out <file>]`
  records the resource's next cycles and saves the trace as a `.trusttrace` file, by default
  `trace-<UTC date and time>.trusttrace` in the current folder, after checking each part's CRC-32
  and the whole trace's SHA-256. It prints the cycles, why the recording ended and the file's size.
  Ctrl+C ends the recording early and saves what it holds (specification 11, "Cycle traces").
- A cycle trace also keeps variable and address forces where they are set and released, not in
  each cycle they hold, `io.write`'s one-shot writes as the bytes they give the process image, and
  a write or force of an instance's member by the path the debugger names it by (specification 48
  RU-59).
- A cycle trace also keeps a tool's method calls (`method.call`), made between two scans, as the
  next cycle's change, with each input's literal as given. A breakpoint or a step that holds the
  program ends the recording `paused` with the cycles before the held one (specification 48 RU-59;
  specification 11, "Cycle traces").
- A cycle trace records each change with where it came from: the control link's, a service's
  `variables.write` as a service's, and an ADS client's write as a protocol server's (specification
  48 Appendix C, C.6).
- A cycle trace also keeps what screens change: each write a screen's command makes, and each
  method a screen calls, with its inputs as Structured Text literals, as the next cycle's change
  of a screen (specification 48 RU-59).
- A cycle trace also keeps Control Studio's experiment writes, of variables and of addresses, as
  Control Studio's changes (specification 48 RU-59).
- A cycle trace names each variable and each method call's instance as the debugger names it,
  whatever form the request named it in: `set main.pumps[2].speed` is recorded as
  `Main.Pumps[2].Speed` (specification 48 Appendix C, C.6).
- A simulator replays a cycle trace (`simulator.replay`, after uploading the trace with
  `transfer.begin {"for": "trace"}`): it takes the recorded memory, runs the recorded cycles on the
  recorded input images with the recorded tasks at their times, and reports the first cycle whose
  outputs or fault differ (specification 48 RU-60). A build whose memory is laid out as the recorded
  program's takes the recorded memory byte for byte. A changed build takes the recorded values as
  an online change carries a running program's, and the notes list what it could not carry over.
  The trace's forces start in effect, each cycle's changes from outside
  (writes and forces of variables and addresses, their releases, and method calls) are applied
  before the cycle as the runtime applied them, and the scan reads the recorded `CURRENT_DT`
  values. A variable, address or method the build lacks is listed once in the notes and skipped.
- trust-sim replays a cycle trace in `cargo test`: `trust_sim::Replay::open(path)?.run(&project)?`
  replays the trace in a simulator of the project as it is now and returns `Ok(())` when every
  cycle's outputs match, and otherwise `Error::Replay` with the report: the first cycle that
  differs, at its logical time, each output that differs with the trace's value and the build's, how
  many later cycles differ, and the notes. A trace recorded at commissioning thereby becomes a test
  of the next change (specification 48 RU-60).
- `trust-dev replay <trace> [--project <path>]` builds the project as it is now and replays a cycle
  trace in a simulator of it. It prints the report and ends with 0 when every cycle matches, with 1
  when a cycle differs, and with 2 and the reason when the trace or the build cannot be replayed
  (specification 48 RU-60).
