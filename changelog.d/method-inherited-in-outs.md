### Fixed

- A method of a function block that extends another may no longer name the base block's `VAR_IN_OUT` or `VAR_TEMP`
  variables, as IEC 61131-3 requires of every method of a block: they belong to the block's body. The analysis
  rejected them only in the methods of the block that declares them, so such code compiled; it now reports
  "function-block method cannot access body-only member", and completion no longer offers them there.
