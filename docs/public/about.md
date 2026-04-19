# About

Use this page when you need the plain-language truth about what truST is, who
maintains it, how to get help, and which surfaces are mature enough for
production evaluation.

## What truST Is

truST is an open-source Structured Text toolchain and runtime platform. It
combines:

- editor tooling
- runtime execution
- debug/control surfaces
- browser IDE and HMI surfaces
- CLI, harness, and agent workflows

## Who Maintains It

- primary public maintainer: Johannes Pettersson
- public project home:
  <https://github.com/johannesPettersson80/trust-platform>

## License

truST is dual-licensed under:

- MIT
- Apache-2.0

## How To Get Help

- [FAQ](faq.md)
- [Troubleshooting](troubleshooting.md)
- GitHub Issues:
  <https://github.com/johannesPettersson80/trust-platform/issues>

## Support Model

- community support is best-effort
- public GitHub issues are the default support path
- site-specific plant runbooks and escalation contacts still belong to the plant
  owner, not the generic truST docs

## Security Contact

If you believe you found a security problem:

- open a private security contact path if one is published for the release
- otherwise use the maintainer contact listed in the repository and avoid
  posting exploit detail publicly before coordination

## Stable Vs Beta

Use the docs and changelog as the public truth source.

General rule of thumb:

- CLI/reference/config contracts documented under [Reference](reference/index.md)
  are the most stable public surfaces
- VS Code authoring is the main engineering workflow
- Browser IDE and some runtime/web flows are real and shipped, but still need
  operator/admin honesty in the docs
- advanced runtime-cloud and experimental workflows should be evaluated against
  their specific docs/examples, not assumed by name alone

## Release Cadence

- use [Changelog](changelog.md) for full release notes
- use [Version History](reference/version-history.md) for "what changed between
  versions?" guidance

## Next

- [FAQ](faq.md)
- [Contribute](contribute.md)
- [Version History](reference/version-history.md)
