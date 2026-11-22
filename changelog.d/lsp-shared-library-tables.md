### Changed

- The language server collects a library's declarations once, in the library's own project, and
  every project that uses the library reads them from there (specification 14 §10.2). On the
  repository's examples and libraries (858 files in 352 projects, many of them using OSCAT),
  opening, changing and closing 100 files took 7.2 s and 236 MB instead of 15.7 s and 402 MB, and
  every file's diagnostics stayed the same.
