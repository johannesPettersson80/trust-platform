# About

truST is an open IEC 61131-3 Structured Text toolchain with editor support, a
runtime you can run locally or on target hardware, and browser UIs for
engineering and operation.

## Maintainer

- Maintainer: Johannes Pettersson
- Project home: <https://github.com/johannesPettersson80/trust-platform>
- Contact: johannes_salomon@hotmail.com

## License

truST is dual-licensed under:

- MIT
- Apache-2.0

## Support Model

- Community support: GitHub issues and public docs
- Direct maintainer contact: email
- Paid support: no formal commercial support contract

## Security Contact

Report security issues privately to:

- `johannes_salomon@hotmail.com`

Do not post exploit details publicly before coordination.

## Known Production Users

- Production users: none disclosed.

## Change Tracking

- Shipped changes: [Changelog](changelog.md)
- Released versions: [Version History](reference/version-history.md)

## Release Cadence

- Releases are published through GitHub Releases
- The changelog and version history pages are the public source of truth for
  shipped changes

## Stable

- core Structured Text authoring workflow
- CLI/reference/config contracts documented under [Reference](reference/index.md)
- VS Code as the primary engineering workflow

## Evaluate Per Site

- Browser IDE in larger teams: confirm how your team will coordinate file
  ownership, reviews, and concurrent edits.
- Browser HMI for production-facing operator use: confirm your auth model,
  alarm handling, and write-enable procedure before live rollout.
- runtime-cloud and multi-runtime federation: test network partitions, auth,
  retries, and recovery behavior on your site topology.
- visual editors outside the documented statechart/editor flows: compare the
  generated ST and runtime behavior against your project requirements.

## Related

- [Installation](start/installation.md)
- [Install On Target](operate/install-on-target.md)
- [FAQ](faq.md)
- [Changelog](changelog.md)
