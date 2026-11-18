### Changed

- A new project's `.gitignore` also lists `/build/`, the folder its builds write, such as the
  Simulator's, so that `git add .` no longer takes them; an existing `.gitignore` gains the line
  when `trust-runtime new --force` writes the project again (specification 25 §3.2).
