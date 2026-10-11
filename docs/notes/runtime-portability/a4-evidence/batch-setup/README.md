# A4 batch setup

These are the exact prepared run-1 command and preparation scripts. Their presence
is not evidence that any command ran. Results and the frozen-source manifest are
recorded separately after execution.

`lease-tools/` contains byte-identical execution tooling from the reviewed A1
integration candidate `b703482bed30a0ef2cf664982c2a4a143962c083`. A4's inherited
lease helper predates mounted-volume support. The reviewed helper enforces mount,
canonical-path and ownership checks for the existing mounted target root; it does
not relax free-space thresholds or bypass a lease. The helper resolves its path
policy relative to its own location.

The planned launch is on `trust-builder`, from the A4 checkout, through the
absolute A1 integration helper path:

```sh
/home/johannes/projects/trust-platform-portability-a1-integration/scripts/with_cargo_target_lease.sh \
  /mnt/HC_Volume_107089260/builder-storage/cargo-targets/trust-portability-a4 \
  bash /home/johannes/.cache/trust-portability-a4-setup-run-1/run-on-builder.sh
```

All Cargo-producing commands remain under that lease. The run script refuses a
second launch when its STARTED marker exists. A1's heavy batch completes before
this command launches; source synchronization does not launch validation.
