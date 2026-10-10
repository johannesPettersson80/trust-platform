# B-R4 firmware boundary independent source review

Reviewer: `/root/br1_static_registry`. Verdict: source accepted after the pre-repaint finding below was corrected. No builds, tests, formatting, links, board commands or source edits performed by this reviewer.

The three fixture entry functions preserve phase boundaries with targeted inline(never); this targets the retained3736-byte combined outer-runner frame but promises no measured saving. Function order, artifact contents, admitted depth4, numeric functions, sampling/scan timing placement and hardware protocol fields remain intact.

The new host-testable stack_guard retains capacity16384 and minimum remaining2048. It rejects saturation, oversized samples, inconsistent sums and arithmetic overflow. Native tests include the exact historic16272/112 and16344/40 failures and acceptance at the2048 boundary. The library export lets these assertions compile independently of HAL hardware.

MEM records now use preserved boot-wide high-water and saturating remaining arithmetic, then enforce the same guard. STACK records still retain per-depth values and immediately fail if their margin is insufficient. The reviewed correction adds a fresh boot-wide guard after engineering-write/wait work and before repaint, so prior transient/logging/engineering peaks cannot be painted away before the next depth test.

Failure propagation is unchanged: phase check returns Err through run; main switches the output off, emits B1,FAIL,runner,stack-headroom and enters the existing stopped idle loop, which keeps outputs safe and feeds the watchdog to prevent automatic reset/retest. A failed check cannot reach deliberate watchdog-arm/reset logic. The guard is sampled detection and does not prevent the first overflow inside an operation; the shared continuation correction and fresh physical evidence are still necessary.

Resolved review finding: the first draft checked only the post-scan local depth sample. repaint_inactive_stack could discover a higher earlier peak and then clear inactive paint before any boot-wide check. Added pre-repaint check closes that boundary without changing thresholds or fixtures.

All source acceptance remains conditional on authorized final software/link/inspection/hardware validation. `source-identity.json` SHA-256: `72c924b048f408a94b31101a5aeb5d58164b4a99e55643b8e3f2d157094dbea1`.
