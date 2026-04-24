# Runtime Cloud Manual

This manual covers cloud-plane runtime communication for multi-runtime operation.

Use this pack for onboarding, profile selection, cross-site federation, fleet UI operation, and troubleshooting.

## Read in Order

1. Quickstart: `docs/guides/RUNTIME_CLOUD_QUICKSTART.md`
2. Profile cookbook (`dev`, `plant`, `wan`): `docs/guides/RUNTIME_CLOUD_PROFILE_COOKBOOK.md`
3. Federation guide: `docs/guides/RUNTIME_CLOUD_FEDERATION_GUIDE.md`
4. UI walkthrough: `docs/guides/RUNTIME_CLOUD_UI_WALKTHROUGH.md`
5. Troubleshooting: `docs/guides/RUNTIME_CLOUD_TROUBLESHOOTING.md`

## Example Pack

- `examples/runtime_cloud/README.md`
- `examples/runtime_cloud/runtime-a-dev.toml`
- `examples/runtime_cloud/runtime-b-dev.toml`
- `examples/runtime_cloud/runtime-plant.toml`
- `examples/runtime_cloud/runtime-wan.toml`
- `examples/runtime_cloud/dispatch-status-read.json`
- `examples/runtime_cloud/preflight-cross-site-deny.json`
- `examples/runtime_cloud/preflight-cross-site-allow.json`

## Scope Notes

- This manual focuses on cloud-plane operations and federation behavior.
- Realtime/T0 timing behavior is documented separately in runtime realtime communication docs.

## Validation Gates

Use the runtime-cloud release gates when validating changes:

- `./scripts/runtime_comms_bench_gate.sh`
- `./scripts/runtime_comms_fuzz_gate.sh`
- `./scripts/runtime_comms_conformance_gate.sh`
- `./scripts/runtime_cloud_security_profile_gate.sh`
- `./scripts/check_zenoh_baseline.sh`
