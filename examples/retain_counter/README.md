# Retain counter

Keep a part counter's total over restarts and stops while its shift count starts over. You learn what
`RETAIN` keeps, which restarts keep it, and how the runtime saves it to a file and reads it back.

## What you learn

- A variable in a `VAR RETAIN` section keeps its value over a warm restart and over a stop of the
  runtime followed by a warm start. A variable in a plain `VAR` section takes its initial value
  again.
- A cold restart gives every variable its initial value, the retained ones too.
- With `[runtime.retain]` set to `mode = "file"` in `runtime.toml`, the runtime saves the retained
  values to a file every `save_interval_ms` and when it stops, and a warm start reads them back. A
  cold start reads no file.

## What you need

- `trust-runtime`, from the truST extension for VS Code or a release archive, and two terminals.
- About 10 minutes.

## Step by step

1. Open a terminal in the folder `examples/retain_counter` and build the project:

    ```text
    trust-runtime build --project .
    ```

    It writes the program's image, here on an x86-64 Linux computer, and then names the install
    file, the key that signed the image and its two sources: `src/Configuration.st`, whose task
    `Count` runs the program `Main` once a second, and `src/Main.st`.

    ```text
    Wrote ./program.tpi
    Wrote ./build/install.x86_64-unknown-linux-gnu.bin for trust-runtime ctl install
    Debug image 2244 bytes: code 842, read-only data 0, starting memory 12
    ```

    `Main` counts one part at each run in two counters: `TotalParts`, declared in a `VAR RETAIN`
    section, and `ShiftParts`, in a plain `VAR` section.

2. Start the runtime in this terminal:

    ```text
    trust-runtime run --project .
    ```

    It runs in the foreground and prints its log lines.

3. In a second terminal in the same folder, after about five seconds, read both counters:

    ```text
    trust-runtime ctl --project . read Main.TotalParts Main.ShiftParts
    ```

    It answers with the scan it read and each value. The task ran at 0 s, 1 s and so on, six runs
    in 5.5 s:

    ```text
    scan=55 time_ns=5500165068 state=running quality=good
    Main.TotalParts : UDINT = 6
    Main.ShiftParts : UDINT = 6
    ```

4. Restart the program warm, as a PLC does after a short stop:

    ```text
    trust-runtime ctl --project . restart warm
    ```

    The runtime queues the restart and gives the change its audit id:
    `{"audit_id":"<id>","id":1,"ok":true,"result":{"status":"restart queued"}}`. Read both counters
    again about 2.5 s later. `TotalParts` went on from 6 to 9, while `ShiftParts` started over and
    counted 3:

    ```text
    Main.TotalParts : UDINT = 9
    Main.ShiftParts : UDINT = 3
    ```

5. Restart it cold, `trust-runtime ctl --project . restart cold`, and read them about 3.5 s later.
   Both started over:

    ```text
    Main.TotalParts : UDINT = 4
    Main.ShiftParts : UDINT = 4
    ```

6. Stop the runtime:

    ```text
    trust-runtime ctl --project . shutdown --exit
    ```

    The runtime saves the retained values to `retain.bin` in the project folder and ends, and the
    first terminal returns.

7. Start it warm in the first terminal:

    ```text
    trust-runtime run --project . --restart warm
    ```

    About 2.5 s later, `TotalParts` goes on from the 4 it saved at the stop, and `ShiftParts` starts
    over:

    ```text
    Main.TotalParts : UDINT = 7
    Main.ShiftParts : UDINT = 3
    ```

8. Stop it again as in step 6, and start it as step 2 does, without `--restart`, which starts cold.
   About 2.5 s later `TotalParts` reads 3: the cold start read no file, and its next save replaced
   the 7 in `retain.bin`.

## What happened

**What a restart keeps** (IEC 61131-3 §6.5.6):

| Declared in | Warm restart, or a warm start after a stop | Cold restart, or a cold start |
|---|---|---|
| `VAR RETAIN` | keeps its value | takes its initial value |
| `VAR` | takes its initial value | takes its initial value |

A function block instance declared `RETAIN` keeps all its variables: a running timer goes on from
where it was, its first call after the restart adding no time.

**Where the runtime keeps them.** The project's `runtime.toml` sets:

```toml
[runtime.retain]
mode = "file"
path = "retain.bin"
save_interval_ms = 1000
```

The runtime writes the retained values to `retain.bin` every second and when it stops, and a warm
start reads them before its first cycle. A power loss loses the changes since the last save, at most
one second's here. A cold start reads no file, and its saves replace the values in it.

**A supervised runtime starts warm.** The systemd units that the runtime archives carry run
`trust-runtime run --project /opt/trust/current --restart warm`, so that a PLC that systemd restarts,
or that comes back after a power loss, keeps its totals. Start a runtime by hand with
`--restart warm` too, unless its retained values should go.

## Try it yourself

- Stop the runtime with Ctrl+C in the first terminal instead of step 6's command. Ctrl+C asks for
  the same stop, so the warm start of step 7 goes on from the saved total.
- End the runtime as a power loss would, with `kill -9` and its process id. The warm start goes on
  from the last save: in one run, the kill came at 7 and the warm start's first run counted 8.
- Move `ShiftParts` into the `VAR RETAIN` section of `src/Main.st` and build again. Step 4's warm
  restart then keeps it as well.

## Reference

- IEC 61131-3 Ed.3 §6.5.6, the `RETAIN` and `NON_RETAIN` qualifiers.
- Specification 10, the rules of retain over a warm restart; specification 11, warm and cold startup
  and the service units.
- `runtime.toml`'s `[runtime.retain]` and the `trust-runtime ctl` commands `read`, `restart` and
  `shutdown`, in the reference of the documentation.
