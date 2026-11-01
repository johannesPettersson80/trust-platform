# Changelog fragments

A pull request with a user-visible change adds one file here instead of editing `CHANGELOG.md`, so
that open pull requests never conflict over the changelog (specification 22, "Change and release
workflow"). Name it after the change, in lower case with hyphens: `live-values-timeout.md`.

It holds Keep a Changelog sections, `### Added`, `### Changed`, `### Deprecated`, `### Removed`,
`### Fixed` or `### Security`, each with its entries. An entry begins with `- ` and continues on lines
indented by two spaces:

```markdown
### Fixed

- When Live Values' I/O state request times out, the panel clears its values and says that it lost
  the connection to the runtime (specification 25 §4.4).
```

`## [Unreleased]` in `CHANGELOG.md` stays empty. The version bump, `scripts/bump_version.py OLD NEW
DATE`, gathers the fragments into the version's section, the newest first, and removes them. CI
checks every fragment and the empty `## [Unreleased]` with `scripts/changelog_fragments.py check`.
