# Rust service

Run a water heater's program in the runtime, and beside it a Rust service that fetches the electricity
price over HTTP and writes the heater's setpoint for it. You learn how a program takes a service's
values only while the service's heartbeat says it runs, and falls back to a safe setpoint when it stops.

## What you learn

- A service is an ordinary Rust program beside the runtime. It reaches the program only over the
  control link, with a service token of its own, and writes only the variables that `writes` in
  `trust.toml` lists.
- The runtime counts the service's heartbeat in the global `price_heartbeat`, and the program gates on
  it with `SERVICE_FRESH`: it takes the service's setpoint while the service is fresh, and its own safe
  setpoint otherwise. The scan never waits for the service.
- The service's bindings name the program's variables with their Rust types, so the compiler checks
  the service against the program.

## What you need

- truST in VS Code, which brings `trust-runtime` and `trust-dev`. No hardware: the program runs with
  simulated I/O.
- Rust from [rustup](https://rustup.rs), with the linker it names for your computer. The service is an
  ordinary Rust program that you build with your own Rust, and its first build downloads its crates,
  `trust-service` among them, from crates.io. truST's toolchain builds programs, not services.
- Three terminals in the project's folder, and about 25 minutes.

## Step by step

1. In the truST side panel, choose **Start from example**, then **Rust service**. Pick a folder under
   **Select destination folder** and keep the name: VS Code opens the copy. In a terminal in that
   folder, run `trust-runtime build --project .` and then `trust-runtime run --project .`, and leave it
   running. The first build needs the truST toolchain: if `build` says it is not installed, run
   `trust-runtime toolchain install` once.
2. In a second terminal, create the service's token:

   ```text
   trust-runtime ctl --project . service-token price --out secrets/price.token --role engineer
   ```

   It prints `id=service-price role=engineer token_file=secrets/price.token`: the token goes into the
   file, never to the screen. The role Engineer lets the service write.
3. In the side panel, open **Target**, choose **This project's runtime**, and press **Connect**. The
   example allows debugging, so Connect attaches: the status bar says `truST: This project's runtime
   connected`, the debug toolbar appears, and **Live Values** opens. Under **Memory**, `Heater.target`
   (`%MD4`) is 55.0, the safe setpoint, and `Heater.service_ok` (`%MX8.0`) is FALSE, as no service
   runs; under **Outputs**, `Heater.heater` (`%QX0.0`) is TRUE, as the simulated tank reads 0.0.
   Under **Inputs**, type 60.0 in the row of `Heater.temperature` (`%ID0`), the tank's temperature,
   and press **Write**: the heater turns off, as the tank is now warmer than its target.
4. In the second terminal, start the stand-in for a price source:

   ```text
   cargo run --manifest-path price-service/Cargo.toml --example stand_in_source -- 38.2
   ```

   It prints `Serving 38.2 EUR/MWh at http://127.0.0.1:47801/price. Type another price and press
   Enter.` and answers each request for the price in JSON, as a price source on the internet does.
5. In a third terminal, run the service: `cargo run --manifest-path price-service/Cargo.toml`. Once it
   is built, it prints `38.2 EUR/MWh: setpoint 65.0 °C` every second. In Live Values
   `Heater.price_setpoint` and `Heater.target` are 65.0, `Heater.service_ok` is TRUE, and the heater is
   on: in the run, half a second after the service started.
6. In the second terminal, type `120` and press Enter. The stand-in says `Now 120 EUR/MWh.`, the
   service prints `120.0 EUR/MWh: setpoint 50.0 °C` at its next fetch, and `Heater.target` is 50.0:
   the heater is off.
7. Stop the service with Ctrl+C in its terminal. About 2 s later `Heater.service_ok` is FALSE and
   `Heater.target` is the safe 55.0 again, while `Heater.price_setpoint` keeps the service's last
   50.0, which the program no longer takes: in the run Live Values still showed TRUE 1.5 s after
   Ctrl+C, and FALSE 2.0 s after.
8. Stop the stand-in and the runtime with Ctrl+C in their terminals.

## What happened

**The service.** `price-service/src/main.rs` connects as the service `price`, with the token file and
the heartbeat period of `[services.price]` in `trust.toml`, to the endpoint that the runtime published
in `.trust-runtime/control.json` when it started, where `trust-runtime ctl` finds it too. Every second
it fetches the price and writes the setpoint for it, `service.set(plant::Heater::price_setpoint, &setpoint)`. After
its first write it starts its heartbeat, which runs on a thread of its own, so that the service's
work never delays it. At the start of its next cycle the runtime writes the setpoint, as the
program's input, and adds one to the global `price_heartbeat` for each heartbeat:
`trust-runtime ctl --project . read price_heartbeat` shows it count up twice a second.

The setpoint rule is the service's, in `price-service/src/price.rs`; below 50 EUR/MWh electricity is
cheap, and the tank stores heat for later:

> setpoint = 65 °C when the price is below 50 EUR/MWh, otherwise 50 °C

Worked with the run: 38.2 is below 50 and gave 65.0; 120 is not and gave 50.0.

**The gate.** At each scan `SERVICE_FRESH` compares `price_heartbeat` with the value it saw before.
The service is fresh once the heartbeat has changed since the program started, and while the time
since its last change, its age, is at most the timeout:

> FRESH = the heartbeat has changed, and age ≤ TIMEOUT = 2 s

While the service is fresh, `target` is its `price_setpoint`; otherwise it is `SAFE_SETPOINT`, 55.0.
The heater keeps the tank between target − 2 °C and the target: with the tank at 60 °C it is on for
the target 65 (60 < 63) and off for 50 and for 55.

Worked with the run: the service sent a heartbeat every 500 ms, so its last one came at most 0.5 s
before Ctrl+C. The program falls back at its first scan after the age passes 2 s; with the 100 ms
task that is between 1.5 s and 2.2 s after Ctrl+C, and Live Values shows it at its next refresh,
about twice a second. The run showed TRUE at 1.5 s and FALSE at 2.0 s.

**Why the service writes before its heartbeat.** It writes its setpoint first, so the program never
takes a setpoint the service has not written as fresh. It also stops its heartbeat when it has no
price (Try it yourself): its heartbeat means that its values are current, not only that its process
runs.

**Why the program keeps the safe setpoint.** The service can stop, fail or lose its network at any
time, and the scan never waits for it, so the program decides what it does without the service. Here
it heats the tank to 55 °C, whatever the price.

**What the service may write.** `writes = ["Heater.price_setpoint"]` in `trust.toml` is the one
variable the service may write: the build checks the path against the program, and the runtime
refuses a write of any other variable. In the bindings, `price-service/src/plant.rs`,
`Heater.price_setpoint` is the only `Writable` handle, so a service that sets another variable does
not compile. The token's role, Engineer, lets the service write at all; a Viewer's token only reads.

## Try it yourself

- Stop the stand-in instead of the service. At its next fetch the service prints `No price from
  http://127.0.0.1:47801/price: ...` and `No heartbeat: the heater uses its safe setpoint.`, and the
  program falls back as before: in the run 2.7 s to 3.3 s after the stand-in stopped, as the fetch
  comes up to a second later.
- Change the program, then write the bindings again: `trust-runtime build --project .`, then
  `trust-dev service bindings price --out price-service/src/plant.rs`. Try
  `service.set(plant::Heater::target, &70.0)` in `main.rs`: it does not compile, as `Heater.target` is
  not in `writes`.
- Use a real price source: set the environment variable `PRICE_SOURCE` to its URL before `cargo run`,
  and change `parse` in `price-service/src/price.rs` to read its answer.
- Run the service as a plant does, under the computer's service manager, so that it starts at every
  boot and again 2 s after it ends: `cargo build --release --manifest-path price-service/Cargo.toml`,
  then `trust-dev service install price`. It needs root, or an administrator on Windows; without them
  it changes nothing and prints the command to run. `trust-dev service uninstall price` removes it.

## Reference

| File | What it holds |
|---|---|
| `src/Main.st` | the program `HeaterControl`: the gate, the target and the heater |
| `src/config.st` | the configuration: the instance `Heater` every 100 ms |
| `src/generated/services.st` | the global `price_heartbeat`, which `trust-dev rust declarations` writes from `trust.toml` |
| `trust.toml` | the service `price`: its package, token file, heartbeat period and `writes` |
| `runtime.toml` | the runtime's settings; it keeps its platform's default control endpoint |
| `price-service/src/main.rs` | the service: connect, fetch, write, heartbeat |
| `price-service/src/price.rs` | the price source's answer and the setpoint rule, with their tests |
| `price-service/src/plant.rs` | the bindings, which `trust-dev service bindings` writes |
| `price-service/examples/stand_in_source.rs` | the stand-in for a price source |
| `price-service/Cargo.toml` | the service's crate, with `trust-service` at this release's version |

- The service's token in `secrets/` gives the service its rights, so `.gitignore` keeps it out of
  version control.
- `cargo test --manifest-path price-service/Cargo.toml` runs the service's tests.
- [Rust POUs](https://johannespettersson80.github.io/trust-platform/develop/rust-pous/): heartbeats of
  services, writing a service and running one.
- Specification 48, "Rust in truST projects" (`docs/specs/48-rust-pous.md`), section 13, and
  `SERVICE_FRESH` in specification 08 (`docs/specs/08-standard-function-blocks.md`).
