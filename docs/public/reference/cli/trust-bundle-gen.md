# `trust-bundle-gen`

`trust-bundle-gen` builds the program image `program.tpi`, with its deployment
record `deployment-record.bin`, for an already prepared runtime bundle
directory, from the bundle's `src/` sources, for this computer's CPU.

## Usage

```text
Usage: trust-bundle-gen --bundle <DIR>
```

## When to use it

Use `trust-bundle-gen` only when you already have a bundle directory and need
the lowest-level standalone build step. It uses the installed compiler
toolchain (or the folder `TRUST_TOOLCHAIN` names) and does not install one;
`trust-runtime toolchain install` installs it.

Most users should use:

```bash
trust-runtime build --project ./my-plc --sources src
```

instead.

## Related

- [trust-runtime](trust-runtime.md)
- [Runtime Model](../../concepts/runtime-model.md)
