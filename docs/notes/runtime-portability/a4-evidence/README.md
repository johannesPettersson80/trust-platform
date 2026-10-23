# A4 historical evidence storage

Run 1–5 results describe their frozen historical source, not the active external-review corrections. Per-run artifact indexes and reports remain unchanged.

Raw scratch files and all five source archives were copied outside the checkout, verified byte-for-byte by SHA-256, and then removed from their original locations. `external-artifacts.json` maps every original path to its preserved external path, size and digest. Resolve the source-archive entries in the original per-run indexes through that map. Test logs, commands, ledgers, manifests and review records remain here for review. This storage move did not execute or regenerate validation.

Future raw artifacts belong under `/home/johannes/projects/.artifacts/runtime-portability-a4/`, with corresponding builder evidence retained separately. The root `.artifacts/` scratch directory is ignored. Physical board execution remains Scope B.
