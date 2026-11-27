# Vendor library stubs

Edit a program written for another vendor's PLC when truST does not ship that vendor's library
blocks: declare the blocks in stub files, name their folder as a library in `trust-lsp.toml`, and
the editor knows them. You learn what the stubs give the editor and the build, and what they do
not give.

## What you learn

- A `[[libraries]]` entry in `trust-lsp.toml` names a folder of Structured Text that the editor
  indexes beside the project's own sources. Here `vendor/siemens` declares Siemens' `TCON`,
  `NORM_X` and `SCALE_X` as function blocks, with their inputs and outputs and no body.
- With the stubs indexed, the editor checks the program against their interfaces: hover, go to
  definition and completion know their names and members, and a call that names an input a stub
  does not declare is an error.
- A runtime build compiles only the folders that `--sources` names, so a build that takes the
  stubs names their folder too. Their empty bodies compute nothing: the build checks names and
  types, not the vendor's behaviour.

## What you need

- VS Code with the truST extension, and `trust-runtime`, from the extension or a release archive.
- This repository, whose `examples/vendor_library_stubs` you copy.
- Linux or macOS, whose shell runs the commands below. On Windows, copy the folder in the
  Explorer and run the build in PowerShell.
- About 15 minutes.

## Step by step

1. From the root of the repository, copy the example, so that your changes stay out of the
   repository, and open the copy in VS Code:

    ```bash
    rm -rf /tmp/vendor-stubs
    cp -R examples/vendor_library_stubs /tmp/vendor-stubs
    code /tmp/vendor-stubs
    ```

2. Open `trust-lsp.toml`:

    ```toml
    [project]
    include_paths = ["src"]
    vendor_profile = "siemens"

    [[libraries]]
    name = "siemens-stubs"
    path = "vendor/siemens"
    version = "0.1.0"
    ```

    `include_paths` holds the project's own sources, `vendor_profile = "siemens"` analyses them
    under the Siemens language rules, and the `[[libraries]]` entry adds the stub folder.

3. Open `src/Main.st`. The Problems view lists nothing for it: the program's three vendor
   blocks resolve to the stubs.

    ```st
    Connect : TCON;
    Normalizer : NORM_X;
    Scaler : SCALE_X;
    ```

4. Hover `TCON` in `Connect : TCON;`. The hover shows the stub's declaration:

    ```st
    FUNCTION_BLOCK TCON
    VAR_INPUT
        REQ : BOOL;
    END_VAR
    VAR_OUTPUT
        DONE : BOOL;
        ERROR : BOOL;
        STATUS : WORD;
    END_VAR
    END_FUNCTION_BLOCK
    ```

    Ctrl+click `TCON`, or press F12 on it: `vendor/siemens/tcon.st` opens at
    `FUNCTION_BLOCK TCON`.

5. On a new line after `Connect(REQ := Request);`, type `Connect.`. Completion offers the block's
   members: `DONE`, `ERROR` and `REQ` as `BOOL`, and `STATUS` as `WORD`. Delete the line again.
   Then, on a new line after `Engineering : REAL;` in the `VAR` block, type `Extra : NO` and
   press Ctrl+Space: completion offers `NORM_X`. Delete that line too.

6. In a terminal, build the program with the stubs:

    ```bash
    cd /tmp/vendor-stubs
    trust-runtime build --project . --sources src --sources vendor/siemens
    ```

    It writes the program's image, here on an x86-64 Linux computer, and then names the image's
    memory figure, the key that signed it and its four sources, the project's two and the
    stubs' two:

    ```text
    Wrote ./program.tpi
    Wrote ./build/install.x86_64-unknown-linux-gnu.bin for trust-runtime ctl install
    Debug image 4300 bytes: code 858, read-only data 16, starting memory 64
    ```

## What happened

**The library.** The editor reads `[[libraries]]` and indexes `vendor/siemens` beside `src`, so
the program's `TCON`, `NORM_X` and `SCALE_X` are the stubs' function blocks. Their names resolve,
and their inputs and outputs are known by name and type. Without the entry, each of the three
declarations reports the error E102, `cannot resolve type 'TCON'` with its own block's name, and
each of the three calls the error E103, `'Connect' is not callable` with its own instance's name.
`vendor_profile = "siemens"` chooses the language rules; it supplies no Siemens library blocks,
which is what the stubs are for.

**The interface.** A stub declares what a vendor's library declares and no more: the block, its
inputs and its outputs. A vendor's compiled library brings no source body either, and the editor
needs none: hover, go to definition and completion read the declaration, and a call is checked
against it.

**The build.** `[[libraries]]` is the editor's. A build compiles the folders that `--sources`
names and nothing else, so this build names `vendor/siemens` beside `src`. The stubs' blocks have
empty bodies, so the program builds and `NORM_X` leaves its `Y` at 0.0: the build checks names
and types, and the vendor's behaviour is still for the migration to supply. The example has no
`runtime.toml` or `io.toml`, so `trust-runtime check`, which checks those files too, refuses it;
the build is its check.

## Try it yourself

- Misspell the input of the `TCON` call, `Connect(REQUEST := Request);`. The Problems view
  reports the error E105, `unknown parameter 'REQUEST'`, as the stub declares only `REQ`. Undo
  the change.
- Add a block of your own. Create `vendor/siemens/my_block.st` with this declaration and save it:

  ```st
  FUNCTION_BLOCK MY_VENDOR_FB
  VAR_INPUT
      Enable : BOOL;
  END_VAR
  VAR_OUTPUT
      Active : BOOL;
  END_VAR
  END_FUNCTION_BLOCK
  ```

  Then add `Station : MY_VENDOR_FB;` after `Engineering : REAL;` in `src/Main.st`. The type
  resolves, so the Problems view shows only the warning W001, `unused variable 'Station'`, until
  the program uses it, and hovering `MY_VENDOR_FB` shows its declaration.

## Reference

- Specification 14 §7.6, `trust-lsp.toml`'s `[[libraries]]` and `vendor_profile`, and §6.1.2,
  completion after `.`.
- The pages Vendor Libraries and Vendor Profile Examples, under Migration While Programming in the
  documentation.
- The `trust-runtime build` command and its `--sources`, in the reference of the documentation.
