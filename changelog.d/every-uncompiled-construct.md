### Changed

- `trust-runtime check` and `trust-runtime build --json` report every construct that the build does
  not compile yet, each as its own `compile` issue at its file, line and column, instead of only the
  first one. The build goes on after such a construct and reports those of every statement and body
  it reaches, in the order of their places; the build still fails and writes no image
  (specification 11 §6.8.1c, §6.8.1d; specification 44 NX-09).
