# Explicit reinstall cookie preparation

Read-only review by `/root/br1_static_registry`; proposal.tcl was authored as a review artifact and not executed. The operation belongs only to the already authorized reinstall after retaining previous failed-run evidence. Keep firmware's unexpected-reset guard unchanged.

PAC 0.16.0 establishes all addresses and bits: stm32f401/mod.rs PWR/RCC/RTC bases; rcc.rs APB1ENR offset0x40 and CSR offset0x74; rcc/apb1enr.rs PWREN bit28; pwr/cr.rs DBP bit8; rtc.rs backup offsets0x50/0x54. Installed OpenOCD target/stm32f4x.cfg loads mem_helper.tcl; its mmw command uses setbits then clearbits, preserving unrelated register bits.

The proposal records RCC flags, enables only PWREN, reads the clock back, enables only DBP, reads PWR_CR back, records the two backup words, invalidates BKP1R before clearing BKP0R and verifies both zero. It neither writes RCC_BDCR nor unlocks calendar registers nor clears reset flags. The following system reset restores ordinary power-access defaults. Either RUNNING_PHASE or emergency phaseFFFF can remain from a failed test and correctly cause firmware to refuse an unsolicited retry.

ST RM0368 Rev6 section5.4.1 requires a dummy PWR_CR read after DBP modification; section17.3.6 excludes BKPxR from the RTC_WPR unlock sequence; section17.6.20 says system reset preserves backup words. [ST reference manual](https://www.st.com/resource/en/reference_manual/rm0368-stm32f401xbc-and-stm32f401xde-advanced-armbased-32bit-mcus-stmicroelectronics.pdf).

Adjacent source issue reported to root: adapter ResetControl::new and emergency_halt currently omit the required post-DBP PWR_CR read. A debugger preparation step must not replace that production-side ordering fix. No production source edits were performed by this reviewer.
