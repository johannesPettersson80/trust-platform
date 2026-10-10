//! Stop bring-up after a measured stack sample misses the existing safety margin.

/// Linker-reserved main stack capacity; independently checked by the ELF inspector.
pub const STACK_BYTES: usize = 16 * 1024;
/// Minimum measured unused stack required before continuing the hardware sequence.
pub const MIN_REMAINING_BYTES: usize = 2 * 1024;

/// Validate a paint sample without allowing saturation or inconsistent accounting.
pub fn check(used: usize, remaining: usize) -> Result<(), &'static str> {
    if used > STACK_BYTES
        || remaining > STACK_BYTES
        || used.checked_add(remaining) != Some(STACK_BYTES)
        || remaining < MIN_REMAINING_BYTES
    {
        Err("stack-headroom")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_consistent_samples_with_the_full_required_margin() {
        assert_eq!(check(0, STACK_BYTES), Ok(()));
        assert_eq!(
            check(STACK_BYTES - MIN_REMAINING_BYTES, MIN_REMAINING_BYTES),
            Ok(())
        );
        for (used, remaining) in [
            (16_272, 112), // First failed phase from the retained physical run.
            (16_344, 40),
            (STACK_BYTES, 0),
            (
                STACK_BYTES - MIN_REMAINING_BYTES + 1,
                MIN_REMAINING_BYTES - 1,
            ),
            (0, MIN_REMAINING_BYTES),
            (usize::MAX, 1),
            (0, usize::MAX),
        ] {
            assert_eq!(check(used, remaining), Err("stack-headroom"));
        }
    }
}
