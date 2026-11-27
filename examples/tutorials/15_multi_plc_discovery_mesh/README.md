# Tutorial 15: Multi-PLC Mesh Sharing and Discovery

Run two runtimes on one machine and share a value between them: PLC-B publishes its lamp output over
the mesh, PLC-A receives it into a variable of its own, and PLC-A finds PLC-B with a search. It
starts from tutorial 13's project.

## What you learn

- `[runtime.mesh] publish` shares a program's variables by their path, such as `P1.LampOut`, and
  `[runtime.mesh.subscribe]` writes a peer's variable into one of this runtime's, naming the peer
  by its resource name: `"PlcB:P1.LampOut" = "P1.RemoteLamp"`.
- Two runtimes on one machine need their own resource names and ports, and a mesh links only to
  the peers its `connect` names and to those that connect to it.
- A search from a runtime finds the runtimes of its mesh; across a network, a shared `auth_token`
  keeps every other runtime out.

## What you need

- Tutorial 13's project in `/tmp/trust-tutorial-13`, as its README builds it, and
  `trust-runtime`, from the truST extension for VS Code or a release archive.
- Linux or macOS, whose shell runs the commands below, and three terminals.
- About 20 minutes.

## Step by step

1. Copy tutorial 13's project twice, one copy for each PLC:

    ```bash
    rm -rf /tmp/trust-plc-a /tmp/trust-plc-b
    cp -R /tmp/trust-tutorial-13 /tmp/trust-plc-a
    cp -R /tmp/trust-tutorial-13 /tmp/trust-plc-b
    ```

2. Give PLC-A a variable for the shared value. A subscription writes the value it receives into a
   variable of the receiving program, so PLC-A's program declares `RemoteLamp`:

    ```bash
    cat > /tmp/trust-plc-a/src/Main.st <<'ST'
    PROGRAM FirstApp
    VAR
        StartCmd : BOOL;
        LampOut : BOOL;
        RemoteLamp : BOOL;
    END_VAR

    LampOut := StartCmd;
    END_PROGRAM
    ST
    ```

3. Configure the two runtimes. Each has its own resource name, web port and mesh port. PLC-B
   publishes `P1.LampOut`, the lamp of its program instance `P1`:

    ```bash
    cat > /tmp/trust-plc-b/runtime.toml <<'TOML'
    [bundle]
    version = 1

    [resource]
    name = "PlcB"
    cycle_interval_ms = 100

    [runtime.control]
    mode = "production"
    debug_enabled = false

    [runtime.web]
    enabled = true
    listen = "127.0.0.1:18102"
    auth = "local"

    [runtime.discovery]
    enabled = true
    service_name = "LineB"
    advertise = false
    interfaces = []

    [runtime.mesh]
    enabled = true
    listen = "127.0.0.1:5212"
    publish = ["P1.LampOut"]
    subscribe = {}

    [runtime.log]
    level = "info"

    [runtime.retain]
    mode = "none"
    save_interval_ms = 1000

    [runtime.watchdog]
    enabled = false
    timeout_ms = 5000
    action = "halt"

    [runtime.fault]
    policy = "halt"
    TOML
    ```

    PLC-A connects to PLC-B's mesh port and subscribes to that variable under PLC-B's resource name,
    `PlcB`, writing it into its own `P1.RemoteLamp`:

    ```bash
    cat > /tmp/trust-plc-a/runtime.toml <<'TOML'
    [bundle]
    version = 1

    [resource]
    name = "PlcA"
    cycle_interval_ms = 100

    [runtime.control]
    mode = "production"
    debug_enabled = false

    [runtime.web]
    enabled = true
    listen = "127.0.0.1:18101"
    auth = "local"

    [runtime.discovery]
    enabled = true
    service_name = "LineA"
    advertise = false
    interfaces = []

    [runtime.mesh]
    enabled = true
    listen = "127.0.0.1:5211"
    connect = ["127.0.0.1:5212"]
    publish = []

    [runtime.mesh.subscribe]
    "PlcB:P1.LampOut" = "P1.RemoteLamp"

    [runtime.log]
    level = "info"

    [runtime.retain]
    mode = "none"
    save_interval_ms = 1000

    [runtime.watchdog]
    enabled = false
    timeout_ms = 5000
    action = "halt"

    [runtime.fault]
    policy = "halt"
    TOML
    ```

    Neither file names a control endpoint: each project keeps its own by default.

4. Build and check both projects:

    ```bash
    trust-runtime build --project /tmp/trust-plc-a --sources src
    trust-runtime validate --project /tmp/trust-plc-a
    trust-runtime build --project /tmp/trust-plc-b --sources src
    trust-runtime validate --project /tmp/trust-plc-b
    ```

    Each build writes its program's image, here on an x86-64 Linux computer:

    ```text
    Wrote /tmp/trust-plc-a/program.tpi
    Wrote /tmp/trust-plc-a/build/install.x86_64-unknown-linux-gnu.bin for trust-runtime ctl install
    Debug image 2092 bytes: code 681, read-only data 0, starting memory 3
    ```

    and each validation answers `Project ok`.

5. Start PLC-B in the first terminal, then PLC-A in the second:

    ```bash
    trust-runtime run --project /tmp/trust-plc-b
    ```

    ```bash
    trust-runtime run --project /tmp/trust-plc-a
    ```

6. In the third terminal, switch PLC-B's input on, then read PLC-B's lamp and PLC-A's copy of it:

    ```bash
    trust-runtime ctl --project /tmp/trust-plc-b io-write %IX0.0 true
    trust-runtime ctl --project /tmp/trust-plc-b read P1.LampOut
    trust-runtime ctl --project /tmp/trust-plc-a read P1.RemoteLamp
    ```

    The write is queued for PLC-B's next cycle. Each `read` names the scan it read at and the
    value; both lamps are on:

    ```text
    scan=70 time_ns=7000187442 state=running quality=good
    P1.LampOut : BOOL = true
    scan=51 time_ns=5100153412 state=running quality=good
    P1.RemoteLamp : BOOL = true
    ```

    Switch the input off with `trust-runtime ctl --project /tmp/trust-plc-b io-write %IX0.0 false`:
    PLC-A's `P1.RemoteLamp` reads `false` again within a second.

7. Search from PLC-A. The first command prints PLC-A's control endpoint; put it in place of
   `<endpoint>` in the second:

    ```bash
    trust-runtime ctl --project /tmp/trust-plc-a endpoint
    trust-runtime discover --only trust_runtime --endpoint <endpoint>
    ```

    The endpoint is PLC-A's own socket, `unix:///tmp/trust-plc-a/.trust-runtime/control.sock`, and
    the search, which lasts 10 seconds, finds PLC-B by its resource name:

    ```text
    Searching from the runtime at unix:///tmp/trust-plc-a/.trust-runtime/control.sock for up to 10.0 s.
    Found PlcB (dev:runtime:d5a80b2ab7ba4cfafc32274d94f30d7f): truST runtimes (confirmed)
                    Runtime found through the mesh without a control endpoint; adopt may require manual endpoint entry.
    truST runtimes  timed out, 1 found
    Search done: 1 device in 10.0 s.
    ```

8. Stop both runtimes:

    ```bash
    trust-runtime ctl --project /tmp/trust-plc-a shutdown --exit
    trust-runtime ctl --project /tmp/trust-plc-b shutdown --exit
    ```

## What happened

**Publish and subscribe.** At each cycle PLC-B sends the variables its `publish` lists to the
runtimes of its mesh. A subscription names the publisher by its resource name and the variable by
its path, `"PlcB:P1.LampOut"`, and writes each value it receives into a variable of its own program,
`P1.RemoteLamp`. The value crossed in less than half a second here, and it stays as it was received
until a new one comes.

**Two runtimes on one machine.** Each runtime has its own resource name, which its peers use, and its
own ports: the web interface on 18101 and 18102 and the mesh on 5211 and 5212. Each project keeps its
own control endpoint, a socket inside its folder, so `ctl --project` reaches each one.

**The connection.** A mesh links only to the peers its `connect` names, here PLC-A to PLC-B's port,
and to the runtimes that connect to its `listen` port; it does not look for other meshes. The two
meshes here listen on this computer only and have no `auth_token`.

**The search.** `discover --endpoint` asks PLC-A to search, and PLC-A finds the runtimes of its
mesh: PLC-B, under its resource name and its runtime ID. The warning says that the search learned no
control endpoint of PLC-B, which adopting it as a device would need. The search also browses the
network with mDNS, on every interface but the loopback (`interfaces = []`), and a runtime announces
itself there only with `advertise = true`; both runtimes here keep `advertise = false`.

**Across a network.** When the runtimes run on different computers, set `listen` to an address on
the plant network and `connect` to the peer's address, and give both meshes the same `auth_token`
(`openssl rand -hex 24`): a peer without it, or with another, cannot join. Set `tls = true` as well:
the mesh then uses the certificate of `[runtime.tls]` (Tutorial 16) and accepts a peer only with a
certificate signed by the authority of `ca_path` for the address it is reached at. Open the mesh port
only between the PLCs.

## Try it yourself

- Subscribe under PLC-B's discovery name instead of its resource name: change `"PlcB:P1.LampOut"`
  to `"LineB:P1.LampOut"` in PLC-A's `runtime.toml` and start PLC-A again. `validate` still
  answers `Project ok`, but `P1.RemoteLamp` stays `false` whatever PLC-B's lamp does: nothing flows,
  and nothing fails, as for a variable that the program does not have. Change it back.
- Add `auth_token = "0123456789abcdef0123456789abcdef0123456789abcdef"` to PLC-B's `[runtime.mesh]`
  only, and start both runtimes again: PLC-A can no longer join, and `P1.RemoteLamp` stays `false`.
  Neither runtime's output says why, so compare the tokens when a value does not arrive. Add the
  same line to PLC-A's `[runtime.mesh]`: the lamp crosses again.
- Stop PLC-B while PLC-A runs. PLC-A's `P1.RemoteLamp` keeps the last value it received: a
  subscriber cannot tell a publisher that is away from one whose value stays. Start PLC-B again:
  PLC-A connects again, and `P1.RemoteLamp` follows PLC-B's lamp, `false` after its start and
  `true` once you switch its input on.

## Reference

- Specification 11, the mesh: `[runtime.mesh]` with `listen`, `connect`, `publish`, `subscribe`,
  `auth_token` and `tls`, and the command line of `trust-runtime discover`.
- Tutorial 16, the runtime's certificate and how a device gets its own token by pairing.
