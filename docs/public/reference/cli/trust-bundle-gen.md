# `trust-bundle-gen`

`trust-bundle-gen` generates `program.stbc` for an already prepared runtime
bundle directory.

## Usage

```text
Usage: trust-bundle-gen --bundle <DIR>
```

## When to use it

Use `trust-bundle-gen` only when you already have a bundle directory and need
the lowest-level standalone bytecode generation step.

Most users should use:

```bash
trust-runtime build --project ./my-plc --sources src
```

instead.

## Related

- [trust-runtime](trust-runtime.md)
- [Runtime Model](../../concepts/runtime-model.md)
