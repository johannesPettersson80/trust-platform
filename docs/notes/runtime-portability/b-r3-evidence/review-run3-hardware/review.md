# B-R3 physical F401 evidence review

Reviewer: `/root/br1_static_registry`. Overall result: hardware acceptance **FAIL**, consistent with the retained xtask verification exit1, "insufficient measured stack headroom". No command was executed against the board during this review.

## Confirmed artifact and installation evidence

Run3 canonical parity/link/inspection all passed. Inspector reports upper span442208 B, free16544 B,160 B above the unchanged16KiB floor; sector0 free224 B; reserved heap72KiB and MSP16KiB. The installed firmware ELF matches the inspected run3 ELF, SHA87d622c71fa6f6dab01d13caef9db2671a174f9ccb575b0398d76389b8f61c5b. OpenOCD reports verifying458360 firmware bytes and14760 application bytes. The application equals the installed sector prefix. Both firmware regions are byte-identical before/after the separate application write. Both checkpoint sectors are byte-identical before/after and match preflash bytes0x8000..0x10000. All installed-sha256 records recompute.

## Partial physical execution observations

The UART contains145 complete lines: one identity,4 IO,12 MEM,3 PREP,101 TRACE,2 STATE,13 NUM,1 NUM_STATE,4 GPIO,3 STACK and1 FAIL. Main logical fields match all101 saved oracle rows exactly; overrun fields are zero. Largest reported main scan is5741us. Numeric state reports scaled15000000ns, command1,40 activations, last nominal deadline1000000000ns,0 overruns and101 samples; largest reported numeric scan9489us. Thirteen numeric bit records are present, including exact REAL3.0/REAL3.75/LREAL8.0 fields. These are trace observations, not a passing complete hardware verification or worst-case timing proof.

GPIO captures one actual PC13-unpressed sample and injected false/true/false input cases; program outputs and PA5 readback follow0/1/0. No manual button transition or optical LED observation is proven. Initial STOP/FAULT output-low records are present but do not establish the later high-to-safe challenges.

The largest reported normal allocation peak is28272 B during main preparation; reported dropped main/numeric live allocations are zero and normal failure counters are zero. Later memory records occur after stack saturation, so these do not close safe-memory acceptance. Value slot reports32B/alignment8 on the actual target.

## Stack failure and incomplete work

The first stack-headroom rejection is already main-instantiate:16272 B used,112 B remaining, below the required2048 B margin. Main-run reports16384/0 before the explicit depth probes. Repainted probes report depth1=9680/6704, depth2=16344/40, depth3=16384/0; depth4 is absent. Saturation is a lower bound on usage, not proof usage stopped at16KiB. Retained failure halt gives MSP0x20012130,7888 B below the reserved floor0x20014000. PC0x0801ea34 falls inside the mapped emergency_halt symbol. This confirms the reserved stack was exceeded; it does not establish the precise panic cause or prove a heap collision.

The final FAIL is a complete CRLF-delimited record. Its code is exactly64 characters because emergency_halt deliberately takes64 code bytes; do not describe it as serial capture truncation. Its displayed metrics536946432/536967904/536944568 are implausible for this device and cannot be used as allocator measurements. Corruption is consistent with the stack evidence, but the exact failure mechanism needs diagnosis.

No depth4, STACK_PEAK, gpio-dropped, high→STOP/FAULT challenge, WATCHDOG_ARM, watchdog-reset boot/second identity, or DONE record exists. Independent watchdog-reset proof, full safe-state challenge, admitted call-depth proof, board stack/heap acceptance and Scope B completion remain open. Capture timeout124 is the planned acquisition bound, not a success indication. The target was subsequently halted without a reset/retry; halt evidence is retained.

No repository changes, product tests, builds, links, resets or hardware commands during review. `evidence-identity.json` SHA-256: `115c82ecc2a7f9c101d25962a8e08b61b52b17e7976f736b425a62b5276a0b61`.
