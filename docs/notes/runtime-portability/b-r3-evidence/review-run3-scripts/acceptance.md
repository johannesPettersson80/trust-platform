# Run 3 focused link/inspection script review

Reviewer: `/root/br1_static_registry`. Accepted as source preparation for the explicitly authorized one additional link and inspection, then conditional hardware. No execution performed for this review.

Compared environment, measure and freeze scripts against run2: candidate paths only. Baseline.map equals the retained run2 firmware map. New link-inspect.sh creates one validation marker, checks canonical parity, invokes measure.sh once, and runs inspection only if that invocation succeeds. measure.sh retains its exact frozen-review digest/preparation prerequisites and one-use measurement marker. Failed link leaves inspection UNRUN; wrapper succeeds only when both link and inspection succeed. No loops, tests or formatter added. Target lease, six-job environment and disabled sccache remain.

Board script is byte-identical to run2 and requires inspected ELF plus approved software marker; the coordinator must carry forward passing run2 behavior evidence, bind the new ELF and honor all hardware prerequisites. This focused wrapper does not reclassify hardware as passed.

Both reviewed linker files still match review-run2-packing's exact hashes. All 144 Rust source/deletion records in the passing run2 frozen manifest remain identical in the active worktree. The new ELF must be linked/inspected before fit is claimed; prior source-level packing review is not measured proof.

No repository edits, builds, tests, formatters, links or hardware commands. `source-identity.json` SHA-256: `25d2f4e6d909a50189aa260350c4ca93390a32d431f284f4f1258969cb89d0e7`.
