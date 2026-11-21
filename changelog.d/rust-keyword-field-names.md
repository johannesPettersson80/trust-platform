### Fixed

- A field of a Rust structure (`#[trust_struct]`) or a variant of a Rust enumeration
  (`#[trust_enum]`) written as a raw identifier of a Rust keyword that Structured Text does not
  reserve, such as `r#loop` or `r#match`, now builds: the enumeration was refused as no IEC
  identifier, and the SDK's macros and the generated code wrote such a name as the keyword itself,
  which did not compile. Structured Text names it `loop` or `match`, as any field or value.
