# PREEMPT_RT Deployment

Linux soft-real-time runtime deployment with `PREEMPT_RT`.

## What This Supports

This path keeps the normal Linux runtime feature set and adds an explicit
real-time deployment/verification contract for:

- `mesh/Zenoh`
- `realtime / T0`
- Linux field/network protocols such as `EtherCAT`, `Modbus TCP`, and `MQTT`
- the normal control/web/HMI surfaces

This is a Linux-only story today. It is not the embedded/native-host port.

## Current Validation Status

Repository status today:

- The Linux runtime exposes a `runtime.realtime` startup profile with kernel,
  scheduler, affinity, and memory-lock verification.
- The control/status surfaces expose the requested and observed RT posture, and
  Prometheus includes `p50` / `p95` / `p99` cycle windows for application-level
  evidence.
- The shipped Linux behavior-lock suites for `mesh/Zenoh`, `realtime / T0`,
  `EtherCAT`, `Modbus TCP`, `MQTT`, and the control/runtime surfaces stay green
  with the RT posture changes in place.

Reference-target status:

- Baseline comparison evidence is captured in-repo on a Raspberry Pi 5 ARM64
  host running Debian kernel `6.12.62+rpt-rpi-2712`; that host is a normal
  `PREEMPT` kernel (`CONFIG_PREEMPT_RT` not set), so those artifacts are
  baseline data, not `PREEMPT_RT` validation
- ARM64 `PREEMPT_RT` evidence: pending capture on the chosen reference board
- x86_64 `PREEMPT_RT` evidence: pending capture on the chosen reference host
- Only hardware + kernel + workload combinations with attached evidence should
  be treated as validated support

## Support Contract

- Same `trust-runtime` binary on normal Linux and `PREEMPT_RT`
- Real-time behavior depends on:
  - exact hardware
  - exact kernel/image
  - exact runtime workload
  - deployment posture
- `PREEMPT_RT` is soft real-time, not a hard-real-time guarantee
- Claims do not automatically transfer between boards, NICs, or kernels
- Prefer wired Ethernet for reference measurements; Wi-Fi is not the reference
  real-time path

Support matrix:

| Class | Status |
| --- | --- |
| Linux `aarch64` edge hosts such as Raspberry Pi-class systems | Implemented; baseline evidence captured on Raspberry Pi 5 non-RT Linux; `PREEMPT_RT` reference-target evidence pending |
| Linux `x86_64` industrial/desktop hosts with wired NICs | Implemented; reference-target evidence pending |
| `mesh/Zenoh`, `realtime / T0`, control/web/HMI surfaces | Behavior-lock coverage green on normal Linux; `PREEMPT_RT` evidence is target-specific and not yet attached to a validated reference host |
| Linux network/fieldbus paths such as `EtherCAT`, `Modbus TCP`, `MQTT` | Implemented on Linux; `PREEMPT_RT` evidence depends on the exact NIC/driver/hardware mix |
| Wi-Fi-heavy deployments, unsupported NICs, and non-RT kernels | Not a validated real-time claim surface |

## Runtime Profile

Add an explicit realtime section to `runtime.toml` on hosts that should run
under `PREEMPT_RT`:

```toml
[runtime.realtime]
enabled = true
require_preempt_rt_kernel = true
lock_memory = true
scheduler = "fifo"
priority = 70
cpu_affinity = [2]
strict = true
```

Meaning:

- `enabled = true`: turn on Linux RT verification
- `require_preempt_rt_kernel = true`: fail if `/sys/kernel/realtime` is not `1`
- `lock_memory = true`: call `mlockall(MCL_CURRENT|MCL_FUTURE)` at runtime start
- `scheduler = "fifo"` / `priority = 70`: required scheduler policy/priority
- `cpu_affinity = [2]`: pin the scheduler thread to CPU 2
- `strict = true`: fail startup if the requested posture cannot be obtained or
  verified

Important:

- `trust-runtime` verifies scheduler policy/priority, but does not set
  `SCHED_FIFO` itself
- set scheduler policy/priority in the service manager (`systemd`) or an
  equivalent launcher
- `trust-runtime` does apply memory locking and scheduler-thread affinity
- `runtime.realtime.cpu_affinity` pins the scheduler thread only; keep mesh,
  web, HMI, and other worker pools off the scan core with `systemd`
  `CPUAffinity=`, cpusets, or equivalent cgroup policy

## systemd Unit

Use the dedicated template:

- [`docs/deploy/systemd/trust-runtime-preempt-rt.service`](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/deploy/systemd/trust-runtime-preempt-rt.service)

Install it as:

```bash
sudo cp docs/deploy/systemd/trust-runtime-preempt-rt.service /etc/systemd/system/trust-runtime.service
sudo systemctl daemon-reload
sudo systemctl enable trust-runtime
sudo systemctl restart trust-runtime
```

Adjust before production use:

- `CPUAffinity=`
- `CPUSchedulingPriority=`
- `ExecStart=`
- any dedicated `User=` / `Group=` policy you require locally
- consider `IOSchedulingClass=` / `IOSchedulingPriority=` if retain writes,
  historian flushes, or file logging share the same storage path
- document whether repeated RT-posture failures should restart forever or
  escalate with `RestartPreventExitStatus=` or an equivalent supervisor policy

## Host Posture

Recommended host baseline:

- `PREEMPT_RT` kernel/image
- CPU governor fixed to performance
- swap disabled or tightly controlled
- wired NIC on a known-good chipset
- isolate the scan core when practical
- move noisy IRQs away from the scan core when practical
- keep heavy UI/diagnostics traffic off the scan core
- keep non-scan worker pools off the scan core; runtime affinity does not
  relocate the whole process for you

## Verify The Kernel

```bash
uname -a
cat /sys/kernel/realtime
```

If `/sys/kernel/realtime` is unavailable, fall back to the kernel config or the
vendor image evidence for that target.

## Verify The Service Posture

Show the systemd RT settings:

```bash
systemctl show trust-runtime \
  -p MainPID \
  -p CPUSchedulingPolicy \
  -p CPUSchedulingPriority \
  -p CPUAffinity \
  -p LimitMEMLOCK \
  -p LimitRTPRIO
```

Inspect the running process:

```bash
pid="$(systemctl show -p MainPID --value trust-runtime)"
chrt -p "$pid"
taskset -pc "$pid"
grep '^VmLck:' /proc/"$pid"/status
```

Inspect the runtime control surface:

```bash
trust-runtime ctl --project /opt/trust/current status
trust-runtime ctl --project /opt/trust/current config-get | jq '.result | {
  realtime_profile: .["realtime.profile"],
  realtime_scheduler: .["realtime.scheduler"],
  realtime_affinity: .["realtime.cpu_affinity"],
  realtime_lock_memory: .["realtime.lock_memory"]
}'
```

`trust-runtime ctl status` now reports a short RT summary:

```text
state=running fault=none rt_profile=preempt-rt rt_active=true
```

## Capture Evidence

Use the shipped validation script:

```bash
PROJECT=examples/plcopen_motion_single_axis_demo \
OUT_DIR=target/gate-artifacts/preempt-rt \
scripts/runtime_preempt_rt_validate.sh
```

The script captures:

- host/kernel evidence
- optional `cyclictest` baseline
- release-mode `trust-runtime bench project` JSON
- service posture evidence when `trust-runtime.service` is running

The summary distinguishes:

- `validation mode: baseline` when kernel evidence does not confirm
  `PREEMPT_RT`
- `validation mode: preempt-rt` when kernel evidence confirms
  `PREEMPT_RT`

Set `TRUST_RT_REQUIRE_PREEMPT=1` when the run must fail on a non-RT kernel.
When this gate is enabled, install `cyclictest` from `rt-tests` and declare a
target-specific `TRUST_RT_CYCLE_P95_MAX_US` threshold. For release evidence,
also set `TRUST_RT_SOAK_SECONDS=3600` or longer so the measured window is at
least one hour.

Example release-evidence invocation:

```bash
PROJECT=examples/plcopen_motion_single_axis_demo \
OUT_DIR=target/gate-artifacts/preempt-rt-rpi5 \
TRUST_RT_REQUIRE_PREEMPT=1 \
TRUST_RT_CYCLE_P95_MAX_US=200 \
TRUST_RT_SOAK_SECONDS=3600 \
scripts/runtime_preempt_rt_validate.sh
```

Tune it with environment variables:

- `PROJECT`
- `OUT_DIR`
- `TRUST_RT_SERVICE`
- `TRUST_RT_REQUIRE_PREEMPT`
- `TRUST_RT_CYCLICTEST_LOOPS`
- `TRUST_RT_CYCLE_P95_MAX_US`
- `TRUST_RT_SOAK_SECONDS`
- `TRUST_RT_MAX_OVERRUNS`

## Failure Behavior

With `strict = true`, runtime startup fails if:

- the requested RT kernel verification fails
- `mlockall` fails
- requested thread affinity cannot be applied
- observed scheduler policy/priority do not match the requested values

With `strict = false`, the runtime starts and records the mismatch in its
realtime status instead.

## Known Limits

- `PREEMPT_RT` reduces latency and jitter, but firmware/SMI/BIOS/PCIe/GPU
  activity can still break your budget
- the reference RT path is Linux + wired Ethernet, not Wi-Fi
- heavy browser/HMI/diagnostic activity can affect timing if you do not isolate
  it
- fieldbus timing quality remains NIC/driver/hardware specific

## Before You Claim RT On A New Target

- confirm the target is actually running a `PREEMPT_RT` kernel with
  `/sys/kernel/realtime`, kernel config evidence, or vendor image evidence
- install `cyclictest` (`rt-tests`) and capture the kernel-level baseline in
  the same evidence bundle as the runtime scan-cycle measurements
- declare a target-specific `TRUST_RT_CYCLE_P95_MAX_US` threshold before
  calling the run validation evidence
- plan and record the soak duration up front; use at least
  `TRUST_RT_SOAK_SECONDS=3600` for release-grade evidence
- capture and keep the exact runtime config, command lines, and artifact bundle
  for the hardware + kernel + workload combination being claimed

## Related

- [Install On Target](install-on-target.md)
- [Supervision](supervision.md)
- [Performance Tuning](performance-tuning.md)
- [Hardware Compatibility](../reference/hardware-compatibility.md)
- [Linux kernel real-time preemption docs](https://docs.kernel.org/core-api/real-time/)
