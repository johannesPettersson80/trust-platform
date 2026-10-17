# STM32F4 platform adapter

Thin adapter for specification 34, Appendix D.4. The firmware owns the shared runtime,
scheduler, supervisor and traces. This crate owns only the chip's peripherals; it does not
depend on the runtime or compiler.

The chip is the STM32F407, selected by the `stm32f407` Cargo feature (the default); a
bare-metal build without it stops with a message:

| Feature | Chip | Firmware | Specification |
|---|---|---|---|
| `stm32f407` (default) | STM32F407, first used on the STM32F407G-DISC1 | `firmware/trust-stm32f407` | Appendix D.4 |

The STM32F401 and its NUCLEO-F401RE firmware were retired on 17 October 2026 (owner); their
scope B evidence stays in git history and the evidence archives (Appendix D.1).

The chip's values are data in `src/chip.rs`: system clock, flash sectors, RAM, device ID,
ports and the pins the firmware keeps. The chip file, `chips/stm32f407.toml`
(specification 34, Appendix H), holds the same values for the tools; the host test
`tests/chip_file.rs` keeps `chip.rs` and the crystal range of `crystal.rs` equal to it. The
parts that need no hardware are host-tested: `chip.rs`, `pins.rs` (pin names such as
`PD12`), `crystal.rs` (the crystal measurement arithmetic) and `baud.rs` (the console
divisor); `cargo test -p trust-platform-stm32f4` runs them. The hardware is in
`src/hardware_f407/`, apart from the timer (`src/hardware/clock.rs`) and the watchdog and
reset marker (`src/hardware/reset.rs`), which stay where the F401 composition shared them.

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
- **Timer.** TIM2 runs at one microsecond per count with ARR=`u32::MAX`. Its single-owner
  rollover extension must be sampled less than 2^32 microseconds apart. Waiting polls
  TIM2; it is deliberately neither WFI nor a low-power claim. An optional 1 kHz SysTick
  calls `systick_interrupt()` from a firmware-owned exception handler, which only
  increments a counter; it runs no PLC or formatting work.
- **Pins by name (TARGET-01).** `P`, a port letter A to K and a number 0 to 15; the chip
  table decides which pins exist (ports A to I) and which the firmware keeps: PA2/PA3
  (console), PA13/PA14 (SWD), PH0/PH1 (crystal). `DigitalIo` owns the nine GPIO ports as
  peripheral tokens and hands out `Input` and `Output` pins by name. An output latches its
  safe level before it becomes an output. `Chip::pin` and `Chip::check_wiring` are `const fn`,
  so the bench firmware asserts its wiring against the chip table in a constant and a wrong
  line fails the build, not the boot.
- **Console.** USART2 on PA2/PA3 at 230400 8N1, at register level because the pin driver
  owns every GPIO port. A formatted record has a 512-byte maximum and a 100 ms deadline,
  including the final transmit; an error can leave a partial record, and the supervisor
  must treat errors as failures. **The DISC1's ST-LINK may not connect its virtual COM port to
  these pins** (UM1472, quoted by an ST community post; the manual itself has not been
  checked): use a 3.3 V USB-UART adapter on PA2 and GND if the cable gives no output.
- **Watchdog and reset marker.** IWDG periods use the nominal 32 kHz LSI, with the actual
  oscillator timing left to device evidence. The watchdog is not frozen by a debugger halt.
  Start is permanent until reset; feeding belongs to the firmware's completed-work policy.
  RTC backup words 0 and 1 hold a phase marker and its complement. A power-on or brownout
  reset invalidates them. They are not PLC retain storage and promise no power-loss
  durability; the firmware also checks IWDGRSTF for an intentional reset.
- **Fault boundary.** `emergency_halt` drives every configured output to its safe level,
  records phase 65535, writes a bounded `B1,FAIL,<code>` record and feeds an already
  running watchdog forever. It never returns to the VM or resets in a loop.
- **Identity.** DBGMCU_IDCODE device 0x413 and 1024 KiB flash
  (`Chip::verify_identity`). The F405, F407, F415 and F417 share the device ID; the build
  selects the chip. The unique ID is rebuilt from the HAL's fields without an unsafe read.
- **Memory.** 1 MiB flash (4 x 16, 1 x 64, 7 x 128 KiB sectors), 128 KiB SRAM and 64 KiB
  CCM (CPU only). Sector 0 holds the vectors, sector 1 the separately installed application
  bundle and sectors 2 and 3 the checkpoint slots; the firmware starts at sector 4.

## Dependencies and safety

The bare-metal ARM dependency versions are exact: stm32f4xx-hal0.23.0,
stm32f4 PAC0.16.0 and cortex-m0.7.9.
The published HAL manifest declares MSRV1.62; the workspace's selected Rust1.99
baseline is retained. Its PAC uses atomics on this atomic-capable M4 target.
Host tests cover the parts that need no hardware (timer and cookie arithmetic, chip data,
pin names, crystal arithmetic, console divisor) and need no HAL.

Owned HAL/PAC access is safe. The adapter has two narrow unsafe blocks: the terminal fault
boundary, whose register views, taken after interrupts are disabled and with no return to
abandoned HAL owners, also reach the nine GPIO ports' BSRR registers; and the one write of
the TIM11 option register that connects the crystal clock to the timer, for which the PAC
has no safe writer. It has no unsafe unique-ID read and no GPIO pin type, and it does not
use the HAL's serial or GPIO types.

## Primary references

- [Published HAL0.23.0 sources](https://docs.rs/crate/stm32f4xx-hal/0.23.0/source/)
  (`Cargo.toml`, `src/rcc/f4`, `src/timer`, `src/watchdog`, `src/signature`).
- [PAC0.16.0 sources](https://docs.rs/crate/stm32f4/0.16.0/source/), STM32F407 register types.
- STM32F407: ST RM0090 (flash organization, RCC, GPIO, timers, IWDG, RTC backup registers
  and electronic signature), ST DS8626 (HSE and HSI characteristics, pin definitions),
  ST UM1472 (STM32F407G-DISC1 LEDs, button and ST-LINK) and ST's HAL and CMSIS device
  headers (`TIM_TIM11_HSE`, the TIM11 option register value that connects HSE_RTC, and the
  memory map).

Implementation and authored native assertions are unverified until the validation batch
and the bench run. Source inspection does not establish compilation, board fit, timing,
watchdog reset or electrical behavior.
