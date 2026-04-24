# CI/CD

Use this page when truST should be checked by a pipeline instead of only by a
developer workstation. The guide below covers the minimum build/validate/test
loop and how to keep machine-readable output useful for release gates.

Success means a pipeline can fail on diagnostics, build errors, validation
errors, or test failures with enough structured output for a maintainer to
route the issue without opening the editor.

Use [Build, Validate, Test](build-validate-test.md) first if you still need to
understand what each local command proves.

Use this page when the same proof needs to run without a person at the keyboard.

## Guide

--8<-- "docs/guides/PLC_CI_CD.md:3"

## Related

- [Build, Validate, Test](build-validate-test.md)
- [Deploy And Rollback](deploy-rollback.md)
- [Generate Project Docs](../develop/generate-project-docs.md)
