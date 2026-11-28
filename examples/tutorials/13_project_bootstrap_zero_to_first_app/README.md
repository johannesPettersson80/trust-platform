# Tutorial 13: A PLC Project from an Empty Folder

Write a PLC project file by file in an empty folder, build it, run it and change it: a lamp that
follows its start command. You learn what each file of a truST project is for and how the program
reaches its inputs and outputs. Tutorials 14 to 23 start from this project.

## What you learn

- A project is four files: the program, the configuration that runs it and binds its variables to
  addresses, `runtime.toml` for the runtime and `io.toml` for its I/O driver.
- `trust-runtime build` compiles the sources into the program image, `validate` checks the project
  as a start would, and `run` runs the image, not the sources.
- `trust-runtime ctl` writes and reads the process image of the running program.

## What you need

- `trust-runtime`, from the truST extension for VS Code or a release archive, on the `PATH`.
- Linux or macOS, whose shell runs the commands below, and two terminals.
- About 20 minutes.

## Step by step

1. Make an empty project folder with a folder for the sources:

    ```bash
    rm -rf /tmp/trust-tutorial-13
    mkdir -p /tmp/trust-tutorial-13/src
    cd /tmp/trust-tutorial-13
    ```

    The first command removes an earlier copy of this tutorial, with any changes you made in it.
    Keep the folder when you are done: the next tutorials copy it.

2. Write the program, a rule you can watch: the lamp follows the start command.

    ```bash
    cat > src/Main.st <<'ST'
    PROGRAM FirstApp
    VAR
        StartCmd : BOOL;
        LampOut : BOOL;
    END_VAR

    LampOut := StartCmd;
    END_PROGRAM
    ST
    ```

3. Write the configuration, which runs the program and binds its variables to addresses:

    ```bash
    cat > src/Configuration.st <<'ST'
    CONFIGURATION FirstConfig
    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : FirstApp;
    VAR_CONFIG
        P1.StartCmd AT %IX0.0 : BOOL;
        P1.LampOut AT %QX0.0 : BOOL;
    END_VAR
    END_CONFIGURATION
    ST
    ```

    The task `Fast` runs the program instance `P1` every 100 ms, and `VAR_CONFIG` puts
    `P1.StartCmd` at the input `%IX0.0` and `P1.LampOut` at the output `%QX0.0`.

4. Write the runtime's configuration:

    ```bash
    cat > runtime.toml <<'TOML'
    [bundle]
    version = 1

    [resource]
    name = "Tutorial13Resource"
    cycle_interval_ms = 100

    [runtime.control]
    mode = "production"
    debug_enabled = false

    [runtime.web]
    enabled = true
    listen = "127.0.0.1:18083"
    auth = "local"

    [runtime.discovery]
    enabled = false
    service_name = "trust-tutorial-13"
    advertise = false
    interfaces = []

    [runtime.mesh]
    enabled = false
    listen = "127.0.0.1:15283"
    publish = []
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

    It names the resource and its cycle, keeps the runtime in production mode, serves the web
    interface on `127.0.0.1:18083`, turns discovery, the mesh and retained values off, and halts
    the program on a fault. The control endpoint is left out, so the runtime takes its platform
    default, a socket in the project's `.trust-runtime` folder.

5. Write the I/O driver's configuration:

    ```bash
    cat > io.toml <<'TOML'
    [io]
    driver = "simulated"
    params = {}

    [[io.safe_state]]
    address = "%QX0.0"
    value = "FALSE"
    TOML
    ```

    The `simulated` driver needs no hardware and keeps each input at the value you write. The safe
    state turns the lamp off when the runtime stops.

6. Build the program:

    ```bash
    trust-runtime build --project . --sources src
    ```

    It writes the program image, here on an x86-64 Linux computer, and then names the image's
    memory figure, the key that signed it and its two sources:

    ```text
    Wrote ./program.tpi
    Wrote ./build/install.x86_64-unknown-linux-gnu.bin for trust-runtime ctl install
    Debug image 2012 bytes: code 681, read-only data 0, starting memory 2
    ```

7. Check the project:

    ```bash
    trust-runtime validate --project .
    ```

    It answers `Project ok`.

8. Start the runtime in the second terminal with `trust-runtime run --project .`. In the first,
   `trust-runtime ctl --project . status` answers `state=running fault=none`, and you can press the
   start button:

    ```bash
    trust-runtime ctl --project . io-write %IX0.0 true
    trust-runtime ctl --project . io-read
    ```

    `io-write` answers `"status":"queued"`: the value is written at the next cycle. In the answer of
    `io-read`, `%IX0.0` (`P1.StartCmd`) and `%QX0.0` (`P1.LampOut`) both read
    `"value":"Bool(true)"`. Write `false` the same way, and both read `Bool(false)`. Stop the
    runtime with `trust-runtime ctl --project . shutdown --exit`.

9. Change the rule and run it again. In `src/Main.st`, write `LampOut := NOT StartCmd;` in place
   of `LampOut := StartCmd;`, then build and start the runtime as in steps 6 and 8. Now `io-read`
   shows `%IX0.0` at `Bool(false)` and `%QX0.0` at `Bool(true)`, and after
   `trust-runtime ctl --project . io-write %IX0.0 true` the lamp goes out: `%QX0.0` reads
   `Bool(false)`. Stop the runtime.

## What happened

**The four files.** The program says what to compute; it names no address. The configuration
says when it runs, in a task, and where its variables are, in `VAR_CONFIG`: the program reaches
the plant only through those addresses. `runtime.toml` describes the runtime around the program,
and `io.toml` the driver that fills the input image and takes the output image.

**The cycle.** Every 100 ms the runtime reads the inputs from the driver into the input image,
runs `P1` once and hands the output image to the driver. A value that `io-write` gives an input is
written at the start of the next cycle, and the simulated driver keeps it there, so the input
stays as you set it until you write it again.

**Build, check, run.** `build` compiles the sources into `program.tpi`, the image this computer
runs, and `validate` checks the configuration files together with that image, as a start does,
without starting. `run` loads the image: a change to the sources reaches the runtime only through
a new build.

**The control endpoint.** `ctl` finds the running runtime through the endpoint file that the
runtime writes in `.trust-runtime`, so `--project .` is all it needs. The web interface of
`runtime.toml` answers on `http://127.0.0.1:18083` and leads to the browser IDE at `/ide`.

## Try it yourself

- Put `LampOut := StartCmd;` back without building, and start the runtime: `io-read` still shows
  `%IX0.0` at `Bool(false)` and `%QX0.0` at `Bool(true)`, the inverted rule of the last build.
  Build, and the lamp follows the start command again.
- Move `src/Configuration.st` out of `src` and build: the build, `validate` and `run` still pass,
  but `io-read` lists `"inputs":[]` and `"outputs":[]`: without `VAR_CONFIG` nothing binds the
  program's variables to addresses. Put it back and build.
- Move `io.toml` out of the folder: `validate` refuses the project with
  ``Error: invalid project folder 'missing io.toml at ./io.toml and no system io config at /etc/trust/io.toml (run `trust-runtime setup` or `trust-runtime`)'``,
  and `run` does not start. Put it back.

## Reference

- Specification 11 §6.5, I/O drivers and the cycle's input and output images, and §6.8.0a, the
  control endpoint and its endpoint file.
- The tutorials index, `examples/tutorials/README.md`: tutorials 14 to 23 copy this project.
