### Fixed

- `MAX`, `MIN`, `LIMIT`, `SEL` and `MUX` of strings now compile and run, comparing the texts as `<` and `>` compare them.
  The build refused them with "a comparison of values that are not scalars" or "a value that is no scalar where a
  scalar is needed".
