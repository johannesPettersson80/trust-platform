# About

truST is an open IEC 61131-3 Structured Text toolchain with editor support, a
runtime you can run locally or on target hardware, and browser UIs for
engineering and operation.

## Project Purpose

truST combines language tooling, runtime execution, debugging, and
browser-hosted operator flows in one open repository.

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

## Roadmap

- Public roadmap anchor: [Changelog](changelog.md)
- Engineering direction and docs scope evolve in the public repository and
  release notes

## Release Cadence

- Releases are published through GitHub Releases
- The changelog and version history pages are the public source of truth for
  shipped changes

## Stable Vs Beta

### Stable

- core Structured Text authoring workflow
- CLI/reference/config contracts documented under [Reference](reference/index.md)
- VS Code as the primary engineering workflow

### Beta Or Evaluate Carefully

- Browser IDE for day-to-day engineering in larger teams: validate session,
  auth, and deployment setup before team rollout.
- Browser HMI for production-facing operator use: verify your site-specific
  alarm, auth, and runbook flows before live use.
- runtime-cloud and multi-runtime federation paths: validate network topology,
  auth, and recovery behavior per site.
- visual-editor coverage outside the statechart/editor paths already
  documented: confirm the generated ST and runtime behavior in your project.

## Related

- [Installation](start/installation.md)
- [Install On Target](operate/install-on-target.md)
- [FAQ](faq.md)
- [Changelog](changelog.md)
