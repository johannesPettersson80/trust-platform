### Fixed

- A based literal such as `16#0001`, or a decimal one that is not negative, is accepted as a bit
  string where no typed operand gives one, as IEC 61131-3 §6.3.2 makes it a bit string literal:
  `SHL(16#0001, n)`, `NOT 16#00FF` and `16#0F AND 16#3C` were refused as no bit strings. The
  expression takes the bit string type where it goes, so `w := SHL(16#0001, n)` for a `WORD`
  `w` shifts 16 bits; a typed input such as `BYTE#16#01` keeps its own type (specification 05
  §5.2.1).
