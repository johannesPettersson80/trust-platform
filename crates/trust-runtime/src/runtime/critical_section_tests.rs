//! A computer's runtime provides the critical section that the compiled cycle's libraries take.

/// The measured heap of `trust-platform-memory`, which the compiled cycle links, takes the
/// critical section, and a library leaves its implementation to the program that links it: here
/// `critical-section`'s `std` feature, which trust-runtime's Cargo.toml selects. Catches a runtime
/// built without one: Linux's linker dropped the code that takes it, and Windows' linker refused a
/// debug build of trust-runtime (29 October 2026). This test takes the critical section itself, so
/// it links on no computer without one.
#[test]
fn a_computer_provides_the_critical_section_its_libraries_take() {
    assert_eq!(critical_section::with(|_| 7), 7);
}
