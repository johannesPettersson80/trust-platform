# Build, Validate, Test

## The Core Loop

```bash
trust-runtime build --project ./my-plc --sources src
trust-runtime validate --project ./my-plc
trust-dev test --project ./my-plc --output json
```

## What each command proves

### `build`

- parses and type-checks ST sources
- resolves project dependencies
- emits `program.stbc`

### `validate`

- validates `runtime.toml`
- validates `io.toml`
- validates the compiled bundle contract

### `test`

- discovers ST tests in the project
- runs them with configurable timeout and output format
- can list tests without executing them

## Worked Example

```bash
trust-runtime build --project ./examples/tutorials/10_unit_testing_101 --sources src
trust-runtime validate --project ./examples/tutorials/10_unit_testing_101
trust-dev test --project ./examples/tutorials/10_unit_testing_101 --output json
```

![Validation success](../assets/images/terminal/validate-success.gif)

*Figure:* A clean `validate` pass against a shipped project. This is the
config/bundle safety gate before runtime troubleshooting.

![Build failure with a syntax error](../assets/images/terminal/build-failure.gif)

*Figure:* A deliberate one-line build break in a temporary project copy. This is
the shape of a compile failure, not placeholder garbage syntax.

## Test JSON Output And JUnit Output

Use JSON output when an agent or CI parser needs machine-readable results:

```bash
trust-dev test --project ./my-plc --output json
```

Use JUnit output when your CI system expects test-report artifacts:

```bash
trust-dev test --project ./my-plc --output junit
```

![JUnit test output](../assets/images/terminal/test-junit.gif)

*Figure:* JUnit XML emitted from the unit-testing tutorial. This is the CI-safe
artifact shape to feed into a test-report parser.

Useful test options:

- `--list`
- `--filter <substring>`
- `--timeout <seconds>`
- `--output human|junit|tap|json`
- `--ci`

## CI Loop

For CI or automation:

```bash
trust-runtime build --project ./my-plc --ci
trust-runtime validate --project ./my-plc --ci
trust-dev test --project ./my-plc --ci --output junit
```

## Typical Failure Pattern

1. `build` fails:
   fix source code first.
2. `build` passes but `validate` fails:
   fix config shape or bundle/runtime mismatch.
3. `build` and `validate` pass but `test` fails:
   logic behavior is wrong even though the project is structurally valid.

## Related

- [Compile, Validate, Reload](compile-validate-reload.md)
- [trust-runtime CLI](../reference/cli/trust-runtime.md)
- [trust-dev CLI](../reference/cli/trust-dev.md)
- [Tutorials](../examples/tutorials.md)
