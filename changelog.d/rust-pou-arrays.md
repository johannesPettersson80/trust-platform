### Added

- A Rust POU's variables may be arrays (specification 48 RU-25): `Array<T, A, N>` holds the `N`
  elements of an `ARRAY[A..A + N - 1] OF T`, indexed from `A` as Structured Text indexes them,
  arrays of arrays and of strings too; `#[init([1, 2, 3])]` gives its leading elements, and an
  index outside the bounds gives `None`, never a panic.
