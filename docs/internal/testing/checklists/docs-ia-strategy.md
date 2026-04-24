# Docs IA Strategy Notes

This note records the strategic decisions behind the public documentation
information architecture so the current structure can be evaluated instead of
argued indefinitely.

## Success Metrics

Use these as the first manual evaluation gates:

1. A CODESYS/TwinCAT user can decide whether truST is worth evaluating in under
   30 minutes by starting at `Migrate` and reaching the relevant compatibility
   page, limitations, and examples without repo search.
2. A developer with no PLC background can install truST and run the first
   tutorial path in under 20 minutes from `Start`.
3. Every top-level section index passes a five-second test: the first screen
   makes the page purpose, recommended first step, and available branches clear.
4. AI positioning remains inspectable: AI claims link to typed tool surfaces,
   contract tests, Agent API boundaries, or an honest scope limit.
5. Operator and technician paths avoid engineering-only distractions in the
   first screen.

## Analytics Decision

No GA4 or external analytics provider is enabled by default.

Reason:

- the docs are open-source and should not add third-party tracking without an
  explicit maintainer privacy decision
- the current IA work is based on evidence from page structure, expected user
  journeys, and public-doc checks
- future analytics should be privacy-reviewed and documented before enabling

Acceptable future options:

- self-hosted privacy-preserving analytics for top pages, search queries,
  no-result searches, and common exits
- GA4 only if the project explicitly accepts the privacy tradeoff and publishes
  that decision in the docs

Until analytics exists, treat IA decisions as hypotheses and revisit them after
manual evaluation against the success metrics above.

## Docs Versioning Strategy For 1.0

Current public docs are latest-only.

Before truST reaches 1.0, decide whether to add versioned docs. Default
recommendation:

- keep latest-only for pre-1.0 while APIs and runtime contracts are still moving
- add versioned docs at 1.0 if users need stable production references for
  older deployed runtimes
- keep `Reference`, `Operate`, and migration compatibility pages versioned
  first; marketing/concept pages can remain latest-oriented unless they contain
  version-sensitive promises
- preserve redirects or stable legacy URLs for moved pages after versioning is
  introduced

## A11y And Mobile Pass

Static expectations:

- every public image has non-empty alt text
- UI pages have captions that explain what the visual proves
- top-level section indexes avoid long ungrouped lists
- deep navigation stays grouped by persona or task so mobile users do not scan a
  flat wall of pages

Manual pre-release spot check:

- desktop docs homepage
- mobile docs homepage
- mobile `Start`
- mobile `Migrate`
- mobile `Operate`
- mobile `Reference > Specifications`

Acceptance:

- top-level intent is visible without opening several nested menus
- tables remain readable or scrollable
- GIFs and screenshots do not dominate the first mobile viewport without
  context
- keyboard focus reaches nav, search, content, and code-copy controls
