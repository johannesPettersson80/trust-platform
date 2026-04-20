# API Lifecycle And Deprecation

## Current Public Contract Anchors

- [CLI reference](cli/trust-runtime.md)
- [Config reference](config/index.md)
- [Agent API overview](agent-api/overview.md)
- [Agent API v1](agent-api/v1.md)
- [Harness protocol](harness/protocol.md)

## Versioning Rule Of Thumb

- if a surface is versioned explicitly, use that versioned page first
- if a surface is not yet versioned explicitly, treat the current Reference page
  plus the Changelog as the public truth

## Deprecation Expectations

Until a stricter written policy is published:

- breaking public changes must be called out in the changelog
- versioned surfaces such as `agent-api/v1` should not change silently
- migration-relevant behavior should be summarized in
  [Version History](version-history.md)

## What To Check Before Upgrading

- [Version History](version-history.md)
- [Changelog](../changelog.md)
- the exact CLI/config/reference pages your automation depends on

## Related

- [Agent API v1](agent-api/v1.md)
- [Harness Protocol](harness/protocol.md)
- [Version History](version-history.md)
