# Construction fixture: large arrays (checklist C1-22)

`src/main.st` declares a global and a program variable of 5,000 DINTs each. `program.stbc` is its
container, built by this tree's `trust-runtime build` on the builder, from the workspace root:

```sh
cargo run --locked -p trust-runtime --bin trust-runtime -- build --project crates/trust-runtime/tests/fixtures/construction/large-array
```

The build also writes `deployment-record.bin`, which the tests do not read and which is not kept.
`trust-runtime-core`'s test `construction_in_place` constructs the program and measures that no
value of either array is built on the heap.
