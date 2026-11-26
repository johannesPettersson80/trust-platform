# Offline Install

## Offline Install Flow

1. build the binaries on a connected machine
2. copy the runtime binary and any required service/template files to USB or SD
3. copy them onto the target host
4. when the target host builds projects itself, install the compiler toolchain
   from its bundle file: `trust-runtime toolchain install --from
   trust-toolchain-linux-arm64.tar.gz` ([The Compiler Toolchain](../start/compiler-toolchain.md))
5. verify the runtime and any system I/O config locally

## Minimum Files To Carry

- `trust-runtime`
- your project folder or deployment bundle
- the toolchain bundle `trust-toolchain-<platform>.tar.gz` of the release, when
  the target host builds projects itself; a host that only runs a program image
  built elsewhere needs none
- service template if you need boot-time supervision
- any local certificates or tokens required by the deployment

## Do Not Forget

- target-host permissions for GPIO or fieldbus
- log retention and restart policy
- a restore path if the storage media fails

## Verification

Before the target is accepted, run the copied runtime with the project bundle on
the isolated host, confirm local status without internet access, and record the
exact binary version and project revision in the site runbook.

## Related

- [Install On Target](install-on-target.md)
- [Supervision](supervision.md)
- [Backup And Restore](backup-and-restore.md)
