### Fixed

- Passing a fixed-size array to an `ARRAY[*]` parameter no longer reports an implicit conversion, in every parameter
  direction: the binding aliases the argument's storage for the call (varying arrays) and converts nothing. Element
  mismatches and concrete bound mismatches keep their errors (specification 28 §2, IEC deviations DEV-040).
