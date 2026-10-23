# A4 run-2 setup (prepared, unrun)

Run 1 failed; its complete ledger is retained separately. The owner authorized
reviewed correction/validation cycles without repeated permission questions.
This batch keeps the same required native, target, lint and architecture gates.
It adds the affected mutation-contract test module and disables Python cache
writes; evidence/STARTED/freeze paths use run 2. No runtime assertions are removed.

Invoke on the builder from the A4 checkout through its now-complete checked-in
`scripts/with_cargo_target_lease.sh`, with target
`/mnt/HC_Volume_107089260/builder-storage/cargo-targets/trust-portability-a4`, and
`bash /home/johannes/.cache/trust-portability-a4-setup-run-2/run-on-builder.sh`.
The lease/path-policy/removal helpers are byte-identical to the reviewed upstream
copies retained under `../batch-setup/lease-tools/`.
