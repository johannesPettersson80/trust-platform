# Tutorial 14: Deploy and Roll Back a PLC Project

Deploy a project into a versioned store, run the runtime on the store's `current` link, deploy a
changed revision, and roll back to the one before, each time seeing which revision the runtime runs.
It starts from tutorial 13's project.

## What you learn

- `trust-runtime deploy --root <store>` copies the built project into a new bundle of the store,
  points the link `current` at it and `previous` at the bundle before, and writes a summary of what
  changed to `deployments/last.txt`.
- A runtime started on `<store>/current` keeps running the program it started with when a deploy
  moves the link; stopping the program and starting it cold loads the bundle that `current` names.
- `trust-runtime rollback --root <store>` points `current` back at `previous`.

## What you need

- Tutorial 13's project in `/tmp/trust-tutorial-13`, as its README builds it, and
  `trust-runtime`, from the truST extension for VS Code or a release archive.
- Linux or macOS, whose shell runs the commands below, and two terminals.
- About 20 minutes.

## Step by step

1. Copy tutorial 13's project and make an empty store for the deployments:

    ```bash
    rm -rf /tmp/trust-tutorial-14-project /tmp/trust-tutorial-14-deploy
    cp -R /tmp/trust-tutorial-13 /tmp/trust-tutorial-14-project
    mkdir -p /tmp/trust-tutorial-14-deploy
    cd /tmp/trust-tutorial-14-project
    ```

2. Build and check revision A, which copies `%IX0.0` to `%QX0.0`:

    ```bash
    trust-runtime build --project . --sources src
    trust-runtime validate --project .
    ```

    The validation answers `Project ok`.

3. Deploy it and read the summary:

    ```bash
    trust-runtime deploy --project . --root /tmp/trust-tutorial-14-deploy
    cat /tmp/trust-tutorial-14-deploy/deployments/last.txt
    ```

    The summary names the new bundle and that there was none before:

    ```text
    Deployment summary:
    current project version: /tmp/trust-tutorial-14-deploy/bundles/project-<time>-<id>-1
    previous project version: none
    runtime.toml changes:
      - new project version (no previous runtime.toml)
    io.toml changes:
      - new project version (no previous io.toml)
    program.tpi: updated
    sources added: Configuration.st, Main.st
    ```

4. Start the runtime on the store's `current` link in this terminal:

    ```bash
    trust-runtime run --project /tmp/trust-tutorial-14-deploy/current
    ```

    In the second terminal, set the input and read the image:

    ```bash
    trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current io-write %IX0.0 true
    trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current io-read
    ```

    `%IX0.0` and `%QX0.0` read `Bool(true)`: revision A copies the input.

5. Make revision B, which inverts the input. In the second terminal, in
   `/tmp/trust-tutorial-14-project`, change the last statement of `src/Main.st` to:

    ```st
    LampOut := NOT StartCmd;
    ```

    Then build, check and deploy it:

    ```bash
    trust-runtime build --project . --sources src
    trust-runtime validate --project .
    trust-runtime deploy --project . --root /tmp/trust-tutorial-14-deploy
    cat /tmp/trust-tutorial-14-deploy/deployments/last.txt
    ```

    The summary names the new bundle, revision A's as the previous one, and what changed:
    `runtime.toml: unchanged`, `io.toml: unchanged`, `program.tpi: updated` and
    `sources modified: Main.st`. The runtime still runs revision A: `io-read` shows `%QX0.0` at
    `Bool(true)`.

6. Stop the program and start it cold, so that it loads the bundle `current` names:

    ```bash
    trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current shutdown
    trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current restart cold
    trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current io-write %IX0.0 true
    trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current io-read
    ```

    `%IX0.0` reads `Bool(true)` and `%QX0.0` `Bool(false)`: revision B runs.

7. Roll back, and stop and start the program cold again:

    ```bash
    trust-runtime rollback --root /tmp/trust-tutorial-14-deploy
    trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current shutdown
    trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current restart cold
    trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current io-write %IX0.0 true
    trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current io-read
    ```

    The rollback answers `Rolled back to project
    /tmp/trust-tutorial-14-deploy/bundles/project-<time>-<id>-1`, revision A's bundle, and
    `%QX0.0` reads `Bool(true)` again. Stop the runtime with
    `trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current shutdown --exit`.

## What happened

**The store.** Each deploy copied the built project, its program image, configuration and sources,
into a bundle of its own under `bundles/`, and moved two links: `current` to the new bundle and
`previous` to the one `current` named before. The store keeps the bundles those two links name, so
a rollback always has the revision before. The project folder you edit stays apart from what
runs.

**The runtime on `current`.** It keeps its control endpoint and its files in the store folder, so
`ctl --project <store>/current` reaches it whichever bundle `current` names. A deploy or a rollback
only moves the link: the program that runs stays the one the runtime started with, so you choose
when the new revision takes over. `shutdown` stops the program and keeps the runtime reachable, and
`restart cold` then loads the bundle `current` names, as a new process would.

**Operating it.** Build and validate before every deploy, keep the store outside the source tree,
read `last.txt` to see what a deploy changed, and practice the rollback before commissioning, not
during an incident.

## Try it yourself

- Deploy revision B again and run `ls /tmp/trust-tutorial-14-deploy/bundles`: the store holds two
  bundles, the new one that `current` names and revision A's, which `previous` names. The deploy
  removed revision B's earlier bundle, which neither link named any more.
- Run `trust-runtime rollback --root /tmp/trust-tutorial-14-deploy` twice: the second rollback
  swaps `current` and `previous` back, so a rollback is undone by another.

## Reference

- Specification 34 §11.2, the program store, and STATE-08, stop and start.
- The `trust-runtime` commands `deploy` and `rollback`, and the `ctl` commands `io-write`,
  `io-read`, `shutdown` and `restart`, in the reference of the documentation.
- Tutorial 13, which builds the project this tutorial starts from.
