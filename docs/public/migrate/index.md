# Migrate Into truST

Use this path when you already have PLC code, vendor habits, or interchange
files and need to evaluate what can move into truST cleanly.

truST is strongest when migration starts from open artifacts and repeatable
checks: import or model the project, validate diagnostics, stub vendor-only
symbols when needed, then prove behavior through build, test, runtime, and HMI
workflows.

## Start With Your Source

| If you have... | Start here | What to expect |
| --- | --- | --- |
| PLCopen XML | [PLCopen](plcopen.md) | ST-complete import/export guidance and Ladder profile notes |
| CODESYS or TwinCAT-style ST | [CODESYS And TwinCAT](codesys-twincat.md) | vendor profiles, formatting expectations, PLCopen interchange, and library stub strategy |
| Siemens/TIA SCL | [Siemens](siemens.md) | SCL compatibility baseline and import tutorial |
| Mitsubishi/GX Works style code | [Mitsubishi](mitsubishi.md) | GX Works compatibility baseline |
| OpenPLC projects | [OpenPLC](openplc.md) | OpenPLC interoperability guidance |
| vendor libraries | [Vendor Libraries](vendor-libraries.md) | symbol/type stubs for authoring, completion, hover, and navigation |

## Compatibility Matrix

| Source | Status | Strongest path | Honest limit |
| --- | --- | --- | --- |
| PLCopen XML with ST bodies | supported | import/export through [PLCopen](plcopen.md), then build and validate | vendor project packages still need review around non-portable metadata and hardware details |
| CODESYS / TwinCAT-style ST | supported for authoring and interchange | `vendor_profile`, PLCopen interchange, and library stubs | not a byte-for-byte clone of every vendor runtime behavior |
| Siemens/TIA SCL | partial migration support | SCL compatibility baseline plus import tutorial | Siemens-specific libraries and project packaging require stubs or manual modeling |
| Mitsubishi/GX Works style ST | partial migration support | compatibility baseline and vendor profile examples | GX Works project/runtime behavior is not fully reproduced |
| OpenPLC projects | partial interoperability | OpenPLC ST-focused guide and PLCopen where available | target/runtime assumptions must be validated in truST |
| Vendor libraries | authoring support through stubs | local symbol/type stubs for completion, hover, navigation, and diagnostics | stubs are contracts for engineering, not full vendor library semantics |

## Migration Workflow

1. Identify the source ecosystem and open the matching compatibility page.
2. Import or copy the source into a normal truST project layout.
3. Set a `vendor_profile` when the style or syntax expectation is vendor-led.
4. Add library stubs for symbols that must be visible but are not shipped as
   runtime semantics.
5. Run diagnostics, build, validate, and tests.
6. Add HMI descriptors or Browser HMI pages only after the code shape is stable.
7. Use [One Project, Every Surface](../concepts/one-project.md) to choose the
   right surface for the next validation step.

## Honest Limits

- Vendor library stubs provide symbol/type contracts for authoring and
  migration. They do not reproduce every vendor runtime behavior.
- PLCopen interchange is the preferred open path, but not every vendor project
  package maps one-to-one into a portable artifact.
- Visual editor files are authoring surfaces over project artifacts. Validate
  the generated or companion behavior through the normal build/runtime path.
- Hardware-dependent behavior still needs target-host validation.

## Proof Points

- [Build, Validate, Test](../operate/build-validate-test.md)
- [PLCopen interoperability](plcopen.md)
- [Vendor profiles](../develop/vendor-profiles.md)
- [Vendor library compatibility](vendor-libraries.md)
- [Examples](../examples/index.md)
