# STM32F401 platform adapter

Thin NUCLEO-F401RE/MB1136 adapter for specification 34 §13.1 Scope B and Appendix D.1.
The firmware owns the shared runtime, scheduler, supervisor and traces. This crate
owns only board peripherals; it does not depend on the runtime or compiler.

## Fixed profile

- HSI feeds the HAL PLL configuration for nominal 84 MHz SYSCLK/HCLK. Clock accuracy
  remains the physical HSI tolerance; no external oscillator is assumed.
- TIM2 runs at one microsecond per count with ARR=`u32::MAX`. Its single-owner
  rollover extension must be sampled less than 2^32 microseconds apart. Waiting
  polls TIM2; it is deliberately neither WFI nor a low-power claim.
- Optional 1 kHz SysTick calls `systick_interrupt()` from a firmware-owned exception
  handler. The handler only increments a counter; it runs no PLC/formatting work.
- PC13 is pulled up and sampled once. `pc13_high` is the physical level;
  `button_pressed` is its active-low normalization. It is not debounced. Synthetic
  fixture input must be labelled separately by the firmware.
- PA5 starts low before UART and user-program initialization. Readback uses **IDR**,
  not ODR. This is an MCU pin observation, not human LED/actuator confirmation.
- USART2 PA2/PA3 uses ST-LINK VCP at **230400, 8N1**, superseding the initial 115200
  proposal at the firmware owner's request. A formatted record has a 512-byte
  maximum and a 100 ms DWT-cycle deadline including final transmit completion.
  Errors can leave a partial record; the supervisor must treat errors as failures.
- IWDG periods use nominal 32 kHz LSI, with actual oscillator timing left to device
  evidence. The watchdog is not frozen by debugger halt. Start is permanent until
  reset; feed belongs to the firmware's completed-work policy.
- RTC backup words 0/1 hold a phase marker and complement. POR/BOR invalidate their
  interpretation. They are not PLC retain storage and do not promise power-loss
  durability. The firmware additionally checks IWDGRSTF for an intentional reset.
- `emergency_halt` disables IRQs, abandons normal owners permanently, forces PA5
  low, records phase65535, attempts a bounded ASCII failure record, and feeds an
  already-running watchdog forever. It never returns to the VM or resets in a loop.

## Dependencies and safety

The bare-metal ARM dependency versions are exact: stm32f4xx-hal0.23.0,
stm32f4 PAC0.16.0, cortex-m0.7.7, embedded-hal-nb1.0.0 and nb1.1.0.
The published HAL manifest declares MSRV1.62; the workspace's frozen Rust1.95
baseline is retained. Its PAC uses atomics on this atomic-capable M4 target.
Host tests contain only the timer/polarity/cookie arithmetic and need no HAL.

Owned HAL/PAC access is safe. Three narrow unsafe blocks remain: factory UID
volatile reads at the documented fixed F401 address; PA5 read-only IDR through a
shared PAC register view because the HAL's push-pull pin API exposes ODR only;
and the terminal emergency register views after interrupts are disabled, with no
return to abandoned HAL owners. No peripheral `steal()` or heap allocation is used.

## Primary references

- [Published HAL0.23.0 sources](https://docs.rs/crate/stm32f4xx-hal/0.23.0/source/)
  (`Cargo.toml`, `src/rcc/f4`, `src/timer`, `src/serial`, `src/watchdog`, `src/signature`).
- [PAC0.16.0 sources](https://docs.rs/crate/stm32f4/0.16.0/source/), STM32F401 register types.
- [ST UM1724 Rev17](https://www.st.com/resource/en/user_manual/dm00105823.pdf),
  §7.6–7.7 LED/button and §7.10 USART2 default ST-LINK routing.
- [ST MB1136 C04 schematic](https://www.st.com/resource/en/schematic_pack/mb1136-default-c04_schematic.pdf),
  board wiring; the actual board revision has not been visually inspected.
- [ST Nucleo BSP](https://github.com/STMicroelectronics/stm32f4xx-nucleo-bsp/blob/main/stm32f4xx_nucleo.c),
  PC13 falling-edge button convention.
- [ST RM0368](https://www.st.com/resource/en/reference_manual/rm0368-stm32f401xbc-and-stm32f401xde-advanced-armbased-32bit-mcus-stmicroelectronics.pdf),
  TIM2/RCC/IWDG/RTC backup registers and electronic signature.

Implementation and authored native assertions are unverified until the complete
Scope B validation batch. Source inspection does not establish compilation, board
fit, timing, watchdog reset or electrical behavior.
