# Tutorial 23: Observability (Historian + Prometheus Metrics)

Record a program's variables to a history file and serve the runtime's metrics to Prometheus, on
tutorial 13's project. You learn how `[runtime.observability]` chooses what the historian records,
how to read its file, and what the Prometheus endpoint reports.

## What you learn

- In `mode = "allowlist"`, the historian records exactly the variables that `include` names, a
  program's variable by its full path: `P1.StartCmd` is the input of the program instance `P1`. It
  takes a sample of each every `sample_interval_ms`.
- The historian appends each sample to `history/historian.jsonl` in the project folder, one JSON
  object a line, with the time and the value's type.
- With the web interface on, `prometheus_path` serves the runtime's metrics in the Prometheus text
  format, the historian's sample total among them.

## What you need

- Tutorial 13's project in `/tmp/trust-tutorial-13`, as its README builds it, and
  `trust-runtime`, from the truST extension for VS Code or a release archive.
- `curl`, and two terminals.
- About 15 minutes.

## Step by step

1. Copy tutorial 13's project, so that its own configuration stays as it is:

    ```bash
    rm -rf /tmp/trust-observability
    cp -R /tmp/trust-tutorial-13 /tmp/trust-observability
    cd /tmp/trust-observability
    ```

2. Open `runtime.toml` and put these two sections in place of its `[runtime.web]` and
   `[runtime.observability]` sections; tutorial 13's project has no `[runtime.observability]` yet,
   so add it:

    ```toml
    [runtime.web]
    enabled = true
    listen = "127.0.0.1:18084"
    auth = "local"
    tls = false

    [runtime.observability]
    enabled = true
    sample_interval_ms = 1000
    mode = "allowlist"
    include = ["P1.StartCmd", "P1.LampOut"]
    history_path = "history/historian.jsonl"
    max_entries = 20000
    prometheus_enabled = true
    prometheus_path = "/metrics"
    ```

    Tutorial 13's configuration runs the program as `P1`, so its input and output are
    `P1.StartCmd` and `P1.LampOut`.

3. Build and check the project:

    ```bash
    trust-runtime build --project . --sources src
    trust-runtime validate --project .
    ```

    The validation answers `Project ok`.

4. Start the runtime in this terminal:

    ```bash
    trust-runtime run --project .
    ```

5. In the second terminal, in the same folder, turn the input on and, after two seconds, off
   again:

    ```bash
    trust-runtime ctl --project . io-write %IX0.0 true
    trust-runtime ctl --project . io-write %IX0.0 false
    ```

    Each write answers that it is queued, for the program's next cycle:
    `{"audit_id":"audit-…","id":1,"ok":true,"result":{"status":"queued"}}`.

6. Read the last samples of the history:

    ```bash
    tail -n 4 history/historian.jsonl
    ```

    The input and the output went on together, and off together:

    ```text
    {"timestamp_ms":1795722702658,"source_time_ns":3200164309,"variable":"P1.StartCmd","value":{"kind":"bool","value":true}}
    {"timestamp_ms":1795722702658,"source_time_ns":3200164309,"variable":"P1.LampOut","value":{"kind":"bool","value":true}}
    {"timestamp_ms":1795722703758,"source_time_ns":4300206751,"variable":"P1.StartCmd","value":{"kind":"bool","value":false}}
    {"timestamp_ms":1795722703758,"source_time_ns":4300206751,"variable":"P1.LampOut","value":{"kind":"bool","value":false}}
    ```

7. Read the metrics that a Prometheus server would scrape:

    ```bash
    curl -s http://127.0.0.1:18084/metrics | grep -E "^trust_runtime_(uptime_ms|faults_total|historian_samples_total)"
    ```

    The values differ from run to run; here, about five seconds after the start:

    ```text
    trust_runtime_uptime_ms 5472
    trust_runtime_faults_total 0
    trust_runtime_historian_samples_total 10
    ```

    Stop the runtime with `trust-runtime ctl --project . shutdown --exit`.

## What happened

**The allowlist.** The historian sampled the program's copy that the scan publishes, the one that
the HMI and the OPC UA server read, once a second, and kept the two variables that `include`
names, so the history holds nothing else. Each sample of the two shares its time: `timestamp_ms` is the
computer's clock, and `source_time_ns` is the runtime's logical time since its start.

**The values.** Each value keeps its type, `{"kind":"bool","value":true}`, and the program copies
`StartCmd` to `LampOut`, so the two are equal in every sample.

**The metrics.** `/metrics` answers in the Prometheus text format: the runtime's uptime, its fault
and overrun totals, cycle and task times, and the historian's totals, here 10 samples, five of each
variable. `auth = "local"` serves them to clients on this computer.

**Before production.** Change the web interface's address, authentication and TLS to what the site
requires before `/metrics` is reachable from the network, set `max_entries` to what the device's
storage holds, keep the allowlist to the variables that operations need, and decide how the history
file is archived.

## Try it yourself

- Set `include = []` and validate again. The runtime refuses an allowlist that names nothing:
  `Error: invalid config 'runtime.toml: runtime.observability.include must not be empty when
  mode='allowlist''`.
- Set `include = ["P1.LampOut"]`, start the runtime again and toggle the input: the new samples
  hold `P1.LampOut` alone.

## Reference

- Specification 11, the runtime historian's recording and export contract.
- `runtime.toml`'s `[runtime.observability]` and `[runtime.web]`, and the `trust-runtime ctl`
  command `io-write`, in the reference of the documentation.
- Tutorial 13, which builds the project this tutorial starts from.
