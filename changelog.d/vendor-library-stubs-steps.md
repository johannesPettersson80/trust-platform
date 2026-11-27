### Fixed

- The vendor library stubs example walks through its steps with what the editor really shows: no problem for the program
  once its `[[libraries]]` entry indexes the stubs, the hover and the definition of `TCON`, completion of the block's
  members and of `NORM_X`, a misspelled input, a stub block of your own, and the build that takes the stub folder. It
  says what the stubs do not give: their empty bodies compute nothing, so the build checks names and types only. CI runs
  its steps through the language server and its build.
