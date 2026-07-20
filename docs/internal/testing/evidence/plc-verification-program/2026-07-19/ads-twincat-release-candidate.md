# ADS/TwinCAT Release-Candidate Check

- Product source commit: `f3bbc8d0e264c9d27bdf6355a444f4403494cb18`
- Test host: `local-linux-aarch64-raspberrypi`
- Target: `192.168.77.11`, AMS Net ID `100.67.6.217.1.1`, AMS port `851`
- Local ADS identity: `192.168.77.10.1.1`
- Result: `1 passed; 0 failed`
- Artifact SHA-256: `7796d1344c6a160ad7a1fe2c4f249a6a64452ee372e959f1baed069da4a865a7`

## Scope

The cataloged `ads_lab_twincat_doctor_records_status_json` device-in-the-loop
test ran against the live TwinCAT PLC. The route, target identity, PLC RUN
state, symbol upload, handle resolution, sum-up read, notification, and symbol
version checks passed. The target exposed 166 symbols and the doctor completed
with exit status 0.

The guarded write probe was deliberately not configured because no reviewed
harmless write symbol was supplied. The doctor therefore reported `partial`
and `production_ready = false`; the test accepts that documented read-only
posture. This evidence proves the read and notification path only. It does not
claim that ADS writes or the complete hardware-lab production-readiness gate
passed.

## Command

```bash
TRUST_DIT_ARTIFACT_DIR="$PWD/target/gate-artifacts/device-in-the-loop-release-candidate" \
TRUST_DIT_ADS_TARGET=192.168.77.11 \
TRUST_DIT_ADS_TARGET_NET_ID=100.67.6.217.1.1 \
TRUST_DIT_ADS_AMS_PORT=851 \
CARGO_INCREMENTAL=0 \
cargo test -p trust-runtime --test device_in_the_loop \
  --no-default-features --features ads-wire,ethercat-wire \
  ads_lab_twincat_doctor_records_status_json -- \
  --ignored --exact --nocapture
```

The generated `ads-doctor.json` was inspected after the passing run and its
digest is recorded above. The artifact remains a machine-local gate artifact;
this committed record is the durable, reviewable result summary.
