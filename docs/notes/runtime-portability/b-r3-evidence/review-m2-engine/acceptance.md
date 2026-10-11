# B-R3 M2 engine collection source review

Reviewer: `/root/br1_static_registry`. Canonical primary AGENTS and complete skills byte parity reverified in the active B checkout before review. Compared the authored M2 slice against retained `b-r3/m1-source/core-src`, then reviewed final outer metadata reservation additions. The review excludes this reviewer's own numeric changes.

Verdict: accepted for the planned measurement; no source blocker found in these 11 files. Not compilation, test or hardware evidence.

Checked invariants:

- Full u32 instance/frame identities and existing lifecycle u16 declaration-cap proof remain intact; ordering, None versus full-width owner, duplicate replacement, snapshots and arbitrary removal remain unchanged.
- Sorted entries reserve geometrically, with complete allocation bytes, relocation and insertion shifts charged. Four logarithmic traversals cover the longest owner-list push path. Replacement performs no growth. Reservation helpers are used on empty outer maps at construction; nested instance payloads retain fallible charged growth.
- New instance metadata errors remove the issued instance and all earlier metadata before returning the original error. IDs stay consumed, never reused. Promotion adds the destination owner entry before changing lifetime; failure rollback restores existing lifetime associations without allocation/fuel.
- Lifecycle mark capacity is charged/reserved after evaluation/lifetime checks and before value publication; there is no intervening mutation of that same collection. Terminal owner retirement scans each affected collection once and still completes after fuel exhaustion.
- Ordinary activation metadata reserves admitted depth D; owner maps, storage-frame metadata and live activation vectors reserve checked 2D to include initializer staging. New tests pin outer no-growth and unchanged live-vector capacity through repeated deepest-admitted calls.
- initialize_frame records its activation on the dispatcher frame before a fallible metadata charge. Root and nested initialization failures leave the frame on the shared unwind stack; retire_frame releases storage/live metadata even if cleanup charging fails, preserving the first error.
- Read every changed test and checked fixture paths/import visibility/cfg boundaries. The metadata fuel test computes exactly the successful prefix charges; failed earlier insertions may leave capacity but no live entries, which subsequent demand recomputes. No assertions weakened.

No source edits, builds, tests, formatters, links or runtime/hardware commands were performed by this reviewer. The measured M2 footprint and software proof remain pending. Formatting after this review requires byte reconciliation.

`source-identity.json` SHA-256: `24defcfc0952d34d7dc8f28ed584c7b5c4546ae8731d14ce77c487c26d6bbd94`.

## Formatted M2 reconciliation

Reviewed the complete formatting delta. Only whitespace, commas and import/module declaration order changed. All 11 accepted files match their saved before-format identities or remained unchanged, and every current file matches M2 formatted-source-manifest.json SHA-256 `a27301e8e9b8d89f9db9b34329508a0274c93b44b04c56768a122b0f8703cf28`. Accepted for the planned measurement; no build/test/formatter execution by this reviewer.

`formatted-identity.json` SHA-256: `5df75104859a77a162752288f93316601be0f377b79f71a0f635b1d1d543933a`.
