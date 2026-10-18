//! The STM32U5A9's independent watchdog interval (specification 34, D.5; design §1.5): the prescaler and
//! the reload that the IWDG takes for a nominal interval on the 32 kHz LSI.

use trust_platform_stm32u5::watchdog::{timing_for_ms, Timing, WatchdogError};

#[test]
fn watchdog_interval_gives_prescaler_and_reload() {
    // Specification 34, D.5 and design §1.5: the LSI is 32 kHz, so one millisecond is 32 ticks, and the
    // interval in ticks is 32 times the milliseconds. A prescaler code P divides the ticks by 4 × 2^P
    // (codes 0 to 6: dividers 4 to 256). The reload is 12 bits, so an interval fits when its counts,
    // the ticks divided by the divider and rounded up, are at most 4,096; the reload is the counts less
    // one. The smallest code that fits is used. Catches a wrong tick rate, a divider off by one code, a
    // reload that is not counts less one, and a 13-bit reload.

    // 1 ms: 32 ticks. Divider 4 gives 8 counts, which fit. Code 0, reload 8 - 1 = 7.
    assert_eq!(
        timing_for_ms(1),
        Ok(Timing {
            prescaler: 0,
            reload: 7
        })
    );

    // 1,000 ms: 32,000 ticks. Divider 4 gives 8,000 counts, too many; divider 8 gives 4,000, which fit.
    // Code 1, reload 4,000 - 1 = 3,999. The timeout is (3,999 + 1) × 8 / 32 = 1,000 ms.
    assert_eq!(
        timing_for_ms(1_000),
        Ok(Timing {
            prescaler: 1,
            reload: 3_999
        })
    );

    // 4,000 ms: 128,000 ticks. Dividers 4, 8 and 16 give 32,000, 16,000 and 8,000 counts, too many;
    // divider 32 gives 4,000, which fit. Code 3, reload 4,000 - 1 = 3,999.
    assert_eq!(
        timing_for_ms(4_000),
        Ok(Timing {
            prescaler: 3,
            reload: 3_999
        })
    );

    // 512 ms: 16,384 ticks. Divider 4 gives exactly 4,096 counts, the largest that fit at code 0, so the
    // reload is the 12-bit maximum, 4,095.
    assert_eq!(
        timing_for_ms(512),
        Ok(Timing {
            prescaler: 0,
            reload: 4_095
        })
    );

    // 513 ms: 16,416 ticks. Divider 4 gives 4,104 counts, one more than fit, so code 0 is refused; divider
    // 8 gives 2,052 counts. Code 1, reload 2,051. The boundary between codes 0 and 1 is here.
    assert_eq!(
        timing_for_ms(513),
        Ok(Timing {
            prescaler: 1,
            reload: 2_051
        })
    );

    // 32,767 ms, the largest interval: 1,048,544 ticks. Dividers 4 to 128 give more than 4,096 counts
    // (128 gives 8,191.75, which rounds up to 8,192). Divider 256 gives 4,095.875, which rounds up to
    // 4,096, the most that fit. Code 6, reload 4,095. Its timeout is 4,096 × 256 / 32 = 32,768 ms, so
    // the watchdog fires one millisecond after the requested interval, never before it.
    assert_eq!(
        timing_for_ms(32_767),
        Ok(Timing {
            prescaler: 6,
            reload: 4_095
        })
    );

    // Zero and one more than the largest interval are refused, before any register is written.
    // Catches an interval of zero that starts a counter with no reload, and a limit one too high.
    assert_eq!(timing_for_ms(0), Err(WatchdogError::Interval));
    assert_eq!(timing_for_ms(32_768), Err(WatchdogError::Interval));
}
