### Changed

- A release publishes the packages that its pull request's rehearsal built and tested when the merge commit on `main`
  has the rehearsed head's tree, as soon as `main` has the commit. Before, `main`'s CI tested the same tree again for
  about two hours and the release built every package again for about one and a half (0.29.0 and 0.30.0). Each
  rehearsal now runs the real-database matrix too and keeps the release's files as `release-bundle-v<version>-<tree>`
  for 30 days; the release checks every file against the rehearsal's `SHA256SUMS` before it publishes. A merge commit
  with another tree is released after `main`'s CI, building every package, as before. `release-provenance.json`
  records the released commit's tree, whether the packages were promoted, and the rehearsal run that built them
  (specification 24, "Release after main CI").
