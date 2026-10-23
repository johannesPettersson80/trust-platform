# B-R4 run1 independent evidence reconciliation

Reviewer `/root/br1_static_registry`; read-only reconciliation after the complete batch ended. No tests, builds, formatter, links, validator reruns or hardware commands were launched. Evidence identity `912a81200a6ad49847a49494513f30e2fd12dfe8b042436ae2faf84eddae6ba5` pins 120 retained raw files; counts are separately retained in counts.json.

The ledger contains 42 steps: 36 required PASS, four required FAIL and two advisory PASS. No required software step is marked unrun. The batch result honestly reports software_required_failed_or_unrun=4. Firmware hardware execution is unrun because the prerequisites failed; no successful software step implies board acceptance.

The 1011-record frozen manifest SHA is ec2cc5372aa76a691df42dc88035de28f7aa9ee38e676bf5f44d38b678d5e0b9, identical to the exact review-approved marker and previously reconciled source review. At this reconciliation the worktree already contains prepared corrections: dispatch.rs, dispatch/continuation.rs, frames.rs, source_free_execution_cost.rs and firmware main/runner/trace differ from the tested freeze; two generated diagram outputs also differ following the batch render. Those source corrections are untested and are not covered by run1. All remaining 1002 manifest records match at this observation.

| Suite | Passed | Failed | Result groups |
| --- | ---: | ---: | ---: |
| Core all features including doctests | 399 | 0 | 29 |
| Core no default features including doctests | 360 | 0 | 29 |
| Core i686 selected suites | 280 | 0 | 8 |
| Hosted unit | 3841 | 0 | 1 |
| Hosted integration | 541 | 1 | 68 |
| Native adapter and xtask | 91 | 0 | 3 (one zero-test doctest group) |
| Native firmware library | 2 | 0 | 1 |
| Provenance helper | 4 | 0 | 1 |

There are no ignored or filtered tests in these reported groups. Rust compile-error text in the core logs belongs to successful compile-fail doctests; it is not a failed suite. Python mutation-tooling/contract checks pass 16 and 20 tests respectively. Advisory metadata reports 1029 validated records. These are metadata maintenance checks, not measured mutation campaigns.

The four failed ledger steps represent three issues: (1) one host fixture fails during compilation because pointer is a reserved keyword; its intended NULL/default/copyback assertion did not execute, so this is not a runtime semantic failure; (2) affected and portable Clippy both report the same manual_clamp pattern at FrameStack growth_demand; (3) the freshly linked firmware fails the unchanged 16 KiB upper-region headroom requirement. All other software steps, including native continuation/unwind/reuse and initializer assertions, physical-deadline behavior, both MCU compile graphs, firmware Clippy, cross-target warnings, both-lock supply-chain gate, architecture, diagrams, formatting, packing and artifact parity, pass in their retained logs.

ELF SHA 1be0a4bf7879d8f82886c5466b63dcd3bec4b2c8ca32273915d0db675e238404 and map SHA 3c4fa4db6f185eafad054d0504c177700bc6c2cff23c962a1a7dd1856a3bf562 match the previously independently inspected copies. Upper span is 442688 bytes, free 16064, shortfall 320 relative to the required 16384. Text 404144, rodata 38544, BSS 73788; sector zero remains 224 bytes free. This is a link success and inspector failure, not an installable or hardware-qualified image. Local native-frame observations and their limitations are retained separately in review-linked-image.
