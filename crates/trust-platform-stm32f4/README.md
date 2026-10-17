# STM32F4 platform adapter

Thin adapter for specification 34 §13.1 Scope B and Appendix D. The firmware owns the
shared runtime, scheduler, supervisor and traces. This crate owns only the chip's
peripherals; it does not depend on the runtime or compiler.

One Cargo feature per chip, and exactly one per bare-metal build (the build stops with a
message otherwise):

| Feature | Chip | Firmware | Specification |
|---|---|---|---|
| `stm32f401` (default) | STM32F401RE on the NUCLEO-F401RE/MB1136 | `firmware/trust-nucleo-f401re` | Appendix D.1 |
| `stm32f407` | STM32F407, first used on the STM32F407G-DISC1 | `firmware/trust-stm32f407` | Appendix D.4 |

What differs between the chips is data in `src/chip.rs`: system clock, flash sectors, RAM,
device ID, ports and the pins the firmware keeps. The STM32F407's chip file,
`chips/stm32f407.toml` (specification 34, Appendix H), holds the same values for the tools;
the host test `tests/chip_file.rs` keeps `chip.rs` and the crystal range of `crystal.rs` equal
to it. The STM32F401 is no longer a supported chip and has no chip file. The parts that need
no hardware are host-tested: `chip.rs`, `pins.rs` (pin names such as `PD12`), `crystal.rs`
(the crystal measurement arithmetic) and `baud.rs` (the console divisor); `cargo test -p
trust-platform-stm32f4` runs them for both chips. The F401 hardware files are the
composition the Scope B evidence was taken with; the F407 has its own tree,
`src/hardware_f407/`, and shares only the timer (`hardware/clock.rs`) and the watchdog and
reset marker (`hardware/reset.rs`).

## STM32F401 fixed profile (feature `stm32f401`)

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

## STM32F407 (feature `stm32f407`)

Firmware is developed for the chip, not for a board (specification 34, ARCH-10): no pin
of any board is in this crate. The board's wiring comes in as a `Wiring`, pin names as
`io.toml` writes them, and `firmware/trust-stm32f407` holds the STM32F407G-DISC1 as its
configuration (LD4 on PD12, B1 on PA0).

- **Clock.** The chip runs from the 16 MHz HSI after reset. The firmware starts the
  crystal and measures it against the HSI: TIM11 input 1 is connected to HSE_RTC, the
  crystal divided by 31, and the timer counts HSI cycles over 64 captures of eight edges
  each. The figure is rounded to a whole MHz (4 to 26 MHz) when it is at most 1.2 % above
  or 5 % below one, and configures the PLL for 168 MHz (APB1 42 MHz, APB2 84 MHz). Above is
  tight because a crystal above the figure the PLL is told would run the chip above 168 MHz;
  for a crystal that is not a whole MHz, with the HSI inside its trim, the chip then runs at
  most about 0.5 % above. No crystal, no clean capture or an unlikely figure puts the PLL
  on the HSI. The startup record
  `B1,CLOCK,<HSE|HSI>,<measured Hz>,<crystal Hz>,<SYSCLK Hz>,<note>` follows each
  `B1,IDENTITY` (the first boot's and the watchdog boot's) and says which. The HSI is
  trimmed to 1 % at 25 degrees (DS8626), so the measurement is no more exact; a crystal
  that is not a whole MHz is named in `device.toml` (TARGET-02).
- **Pins by name (TARGET-01).** `P`, a port letter A to K and a number 0 to 15; the chip
  table decides which pins exist (ports A to I) and which the firmware keeps: PA2/PA3
  (console), PA13/PA14 (SWD), PH0/PH1 (crystal). `DigitalIo` owns the nine GPIO ports as
  peripheral tokens and hands out `Input` and `Output` pins by name. An output latches its
  safe level before it becomes an output. `Chip::pin` and `Chip::check_wiring` are `const fn`,
  so the bench firmware asserts its wiring against the chip table in a constant and a wrong
  line fails the build, not the boot.
- **Console.** USART2 on PA2/PA3 at 230400 8N1, at register level because the pin driver
  owns every GPIO port. **The DISC1's ST-LINK may not connect its virtual COM port to
  these pins** (UM1472, quoted by an ST community post; the manual itself has not been
  checked): use a 3.3 V USB-UART adapter on PA2 and GND if the cable gives no output.
- **Fault boundary.** `emergency_halt` drives every configured output to its safe level,
  records phase 65535, writes a bounded `B1,FAIL,<code>` record and feeds an already
  running watchdog forever, as on the F401.
- **Identity.** DBGMCU_IDCODE device 0x413 and 1024 KiB flash
  (`Chip::verify_identity`). The F405, F407, F415 and F417 share the device ID; the build
  selects the chip. The unique ID is rebuilt from the HAL's fields without an unsafe read.
- **Memory.** 1 MiB flash (4 x 16, 1 x 64, 7 x 128 KiB sectors), 128 KiB SRAM and 64 KiB
  CCM (CPU only). The first four sectors are laid out as on the F401.

## Dependencies and safety

The bare-metal ARM dependency versions are exact: stm32f4xx-hal0.23.0,
stm32f4 PAC0.16.0, cortex-m0.7.9, embedded-hal-nb1.0.0 and nb1.1.0.
The published HAL manifest declares MSRV1.62; the workspace's selected Rust1.99
baseline is retained. Its PAC uses atomics on this atomic-capable M4 target.
Host tests cover the parts that need no hardware (timer, polarity and cookie arithmetic,
chip data, pin names, crystal arithmetic, console divisor) and need no HAL.

Owned HAL/PAC access is safe. The F401 composition has three narrow unsafe blocks: factory UID
volatile reads at the documented fixed F401 address; PA5 read-only IDR through a
shared PAC register view because the HAL's push-pull pin API exposes ODR only;
and the terminal emergency register views after interrupts are disabled, with no
return to abandoned HAL owners. No peripheral `steal()` or heap allocation is used.

The F407 composition has two: the same terminal fault boundary (its register views also
reach the nine GPIO ports' BSRR registers), and the one write of the TIM11 option register
that connects the crystal clock to the timer, for which the PAC has no safe writer. It has
no unsafe unique-ID read and no GPIO pin type, and it does not use the HAL's serial or
GPIO types.

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
- STM32F407: ST RM0090 (flash organization, RCC, GPIO), ST DS8626 (HSE and HSI
  characteristics, pin definitions), ST UM1472 (STM32F407G-DISC1 LEDs, button and ST-LINK)
  and ST's HAL and CMSIS device headers (`TIM_TIM11_HSE`, the TIM11 option register value
  that connects HSE_RTC, and the memory map).

Implementation and authored native assertions are unverified until the complete
Scope B validation batch. Source inspection does not establish compilation, board
fit, timing, watchdog reset or electrical behavior.
