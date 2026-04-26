# PLCopen Motion Library Packages

The reusable PLCopen motion library source of truth lives here.

Packages:

- `single_axis_core`
- `synchronization`
- `coordinated_motion`
- `homing`
- `oop`

Each package contains its own `trust-lsp.toml` and `src/` tree.

The `oop` package is the PLCopen object-oriented facade. It exposes `itfAxis`,
command interfaces, command objects, and `MC_OopAxis`, while delegating axis
behavior to the classic single-axis package.

The fixture projects under
`crates/trust-runtime/tests/fixtures/plcopen_motion/` are conformance consumers
of these packages via `[dependencies]`; they are not the library source.
