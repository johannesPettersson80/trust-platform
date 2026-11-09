### Added

- CODESYS's `{attribute 'qualified_only'}` and `{attribute 'strict'}` take effect:
  - `qualified_only` on an enumeration makes a bare value (`MANUAL`) an error; write
    `E_Mode.MANUAL` or `E_Mode#MANUAL`. On a namespace's `VAR_GLOBAL` list it makes a global that a
    `USING` brings in an error when named bare; write `GVL_IO.lrLevel`.
  - `strict` on an enumeration makes arithmetic with its values an error, and assigning to it a
    constant that is none of its values or a variable of another type.
  Before, both attributes were read and had no effect.
