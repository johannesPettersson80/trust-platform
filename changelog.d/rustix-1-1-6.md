### Security

- truST now builds with `rustix` 1.1.6. RUSTSEC-2026-0486 marks 1.1.5 unsound: its extended-attribute functions could
  mark unwritten memory as initialized when given an empty buffer. Neither truST nor any crate it builds with calls
  those functions, so the packages of 0.37.0 never reached that code.
