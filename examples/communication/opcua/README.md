# OPC UA: A PLC's Variables in an OPC UA Client

Serve two variables of a running PLC program over OPC UA and read them with an OPC UA client: a
tank level that the program moves and a pump state that follows it. You learn how
`[runtime.opcua]` chooses what the runtime's OPC UA server publishes, where a client finds it, and
why this example's open security settings belong on a workbench only.

## What you learn

- `[runtime.opcua]` turns on the runtime's OPC UA server, names its endpoint and namespace, and
  lists in `expose` the global variables that it publishes.
- A client finds the published variables in the folder `truST` under `Objects`, each as
  `ns=2;s=<name>` in the namespace that `namespace_uri` names, and reads their current values. The
  server refuses writes.
- This example serves without security and with anonymous access, for a workbench. The server's
  own defaults are the secure profile `Basic256Sha256` with `SignAndEncrypt` and no anonymous
  access.

## What you need

- This repository, whose `examples/communication/opcua` you copy, and `trust-runtime`, from the
  truST extension for VS Code or a release archive, on the `PATH`. Every release serves OPC UA.
- An OPC UA client. The steps use the command-line tools of the Python package `asyncua`, which
  `python3 -m pip install asyncua` installs (version 2.1.0 here); a graphical client such as
  UaExpert connects the same way.
- Linux or macOS, whose shell runs the commands below, and two terminals. About 15 minutes.

## Step by step

1. From the root of the repository, copy the project:

    ```bash
    rm -rf /tmp/trust-opcua
    cp -R examples/communication/opcua /tmp/trust-opcua
    cd /tmp/trust-opcua
    ```

2. Read the program. `src/config.st` declares two global variables, `TankLevel : REAL := 42.5`
   and `PumpRunning : BOOL`, and `src/main.st` moves them every 100 ms cycle:

    ```st
    TankLevel := TankLevel + 1.0;
    IF TankLevel > 80.0 THEN
        TankLevel := 20.0;
    END_IF;

    PumpRunning := TankLevel > 50.0;
    ```

    The level climbs by 1.0 each cycle and falls back to 20.0 above 80.0, so it runs through its
    range every six seconds, and the pump runs while the level is above 50.0.

3. Read the server's settings, the section `[runtime.opcua]` of `runtime.toml`:

    ```toml
    [runtime.opcua]
    enabled = true
    listen = "127.0.0.1:4840"
    endpoint_path = "/"
    namespace_uri = "urn:trust:runtime"
    publish_interval_ms = 250
    max_nodes = 128
    expose = ["TankLevel", "PumpRunning"]
    security_policy = "none"
    security_mode = "none"
    allow_anonymous = true
    ```

    The server listens on the loopback address at OPC UA's port 4840, at the endpoint
    `opc.tcp://127.0.0.1:4840/`. It publishes the two variables that `expose` names, at most 128,
    in the namespace `urn:trust:runtime`, and refreshes their values every 250 ms. The last three
    lines turn security off and let any client in.

4. Build the program and check the project:

    ```bash
    trust-runtime build --project . --sources src
    trust-runtime validate --project .
    ```

    `validate` answers `Project ok`: it checks the server's settings too.

5. Start the runtime in the second terminal with `trust-runtime run --project .`. In the first,
   browse the server's objects:

    ```bash
    uals -u opc.tcp://127.0.0.1:4840/ -n i=85
    ```

    Among `Objects`, beside OPC UA's own `Server`, the runtime's folder `truST` has the node
    `ns=2;i=0`. Browse it:

    ```bash
    uals -u opc.tcp://127.0.0.1:4840/ -n "ns=2;i=0"
    ```

    ```text
    Browsing node ns=2;i=0 at opc.tcp://127.0.0.1:4840/

    DisplayName                    NodeId                    BrowseName                Value

    LocalizedText(Locale=None, Text='TankLevel') ns=2;s=TankLevel          0:TankLevel              , 21.0
    LocalizedText(Locale=None, Text='PumpRunning') ns=2;s=PumpRunning        0:PumpRunning            , False
    ```

    The values are those of the moment you browse.

6. Read the level and the pump a few times:

    ```bash
    uaread -u opc.tcp://127.0.0.1:4840/ -n "ns=2;s=TankLevel"
    uaread -u opc.tcp://127.0.0.1:4840/ -n "ns=2;s=PumpRunning"
    ```

    Three reads about two seconds apart gave `28.0` and `False`, `46.0` and `False`, then `63.0` and
    `True`: the level climbs, and the pump runs once it passes 50.0. Each command also prints
    `WARNING:asyncua.client.client:Requested session timeout to be 3600000ms, got 60000ms instead`
    on its error output: the server grants sessions one minute, which a read does not need.

7. Try to write the level:

    ```bash
    uawrite -u opc.tcp://127.0.0.1:4840/ -n "ns=2;s=TankLevel" -t float 10.0
    ```

    The server refuses: `The access level does not allow writing to the Node.(BadNotWritable)`.
    Stop the runtime with `trust-runtime ctl --project . shutdown --exit`.

## What happened

**Published values.** After each cycle the runtime keeps a snapshot of the program's variables,
and the server publishes from it, refreshing the exposed variables every `publish_interval_ms`.
Each variable has the read access only: a client's write is refused, as the runtime has no path
yet that would apply a write between two scans (specification 11, "OPC UA server surface").

**Node identifiers.** Namespace 0 is OPC UA's own, namespace 1 the server's application,
`urn:trust:runtime:CommOpcUaRes` after the resource, and namespace 2 the one that
`namespace_uri` names. A variable's node identifier is its name in namespace 2,
`ns=2;s=TankLevel`, and the `REAL` and the `BOOL` arrive as OPC UA's `Float` and `Boolean`.

**What is exposed.** Only the variables that `expose` lists are published, so a client sees what
the commissioning engineer chose and nothing else of the program. Name each variable; a pattern
such as `["*"]` would publish every global at once.

**Security.** With `security_policy = "none"`, `security_mode = "none"` and
`allow_anonymous = true`, anyone who reaches the port reads the values unencrypted. That suits a
workbench, where the server listens on the loopback address alone. On a plant network, set
`security_policy = "basic256sha256"` with `security_mode = "sign_and_encrypt"`, and
`allow_anonymous = false` with a `username` and a `password`; those are the server's defaults when
the lines are left out.

## Try it yourself

- Publish less: write `expose = ["TankLevel"]`, build and start again. The folder `truST` lists
  `TankLevel` alone, and reading `ns=2;s=PumpRunning` answers
  `The node id refers to a node that does not exist in the server address space.(BadNodeIdUnknown)`.
- Close the door without a key: write `allow_anonymous = false` and nothing else.
  `trust-runtime validate --project .` refuses the project with
  `Error: invalid config 'runtime.toml: runtime.opcua requires anonymous access or username/password when enabled'`,
  and `run` does not start.
- Take a security policy without a message mode: write `security_policy = "basic256sha256"` and
  leave `security_mode = "none"`. `validate` refuses
  `unsupported runtime.opcua security profile Basic256Sha256/None`: a policy that signs or
  encrypts needs `sign` or `sign_and_encrypt`.

## Reference

- Specification 11, "OPC UA server surface, security, and cold start": what the server publishes,
  its access and its security profiles.
- `examples/communication/README.md`, the other protocols, and the OPC UA client of a runtime,
  which reads another server's nodes into the program.
