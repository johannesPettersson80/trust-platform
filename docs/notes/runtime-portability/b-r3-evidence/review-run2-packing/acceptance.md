# Immutable-table packing source review

Reviewer: `/root/br1_static_registry`. Canonical AGENTS/full skills parity verified in active B. Verdict: accepted as a source-only linker layout change; no measured gain or new fit claim.

Each selector matches exactly one complete input section in the retained run-2 map: libm generic sqrt RSQRT_TAB, 256 bytes/alignment 2 at 0x080768d0; core unicode white_space WHITESPACE_MAP, 256 bytes/alignment 1 at 0x08076f98. Selectors name these tables without pinning compiler hashes or anonymous ordinals. Input map/ELF/footprint hashes recompute to the proposal identity.

Read the existing ELF section headers with readelf -SW: .rodata is PROGBITS/AMS with neither W nor X, and .firmware_rodata is PROGBITS/AM/alignment32. This is inspection of the previously linked artifact, not execution of firmware or a new link. Both selected tables are immutable data; relocation moves complete sections and leaves functions/capabilities intact.

The actual source appends them after the existing ALIGN(32), starting at the measured 0x08003d20 boundary. Required alignments divide32 and each section is a multiple of32, so predicted end is 0x08003f20, leaving224 bytes before sector1. The added exact512-byte span assertion fails if the expected selection changes. Existing sector0, .text origin0x08010000, flash-end, vector alignment and RAM/stack assertions remain. Application sector1 and checkpoint sectors2/3 are untouched. Vector/data gap and firmware code origin are unchanged.

Arithmetic on the retained image predicts upper free16032+512=16544 bytes, 160 above the unchanged16384-byte floor. Linker layout and literal relocation can affect the actual result; only a newly authorized link and unchanged ELF inspection establish final fit. No hardware installation is approved by this source review.

No repository edits, builds, tests, formatting, links or board commands performed. `source-identity.json` pins both proposal files; SHA-256 `a1d63a34780cca95e256e3e9116d69ffa419e8af0c5ba901e5155906e94e77a7`.
