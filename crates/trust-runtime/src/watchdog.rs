//! Watchdog and fault policy compatibility surface.

pub use trust_runtime_core::watchdog::*;

/// Host action when hosted hang supervision finds a scan thread that did not return
/// (specification 11 section 6.6, "Hosted hang supervision"; specification 34 TIME-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HangAction {
    /// Record the fault and keep the process serving the control plane.
    #[default]
    Fault,
    /// Record the fault, then exit the process with status 70. No destructors run.
    Abort,
}

impl HangAction {
    /// Parse `runtime.watchdog.hang_action` from runtime configuration text.
    pub fn parse(text: &str) -> Result<Self, crate::error::RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "fault" => Ok(Self::Fault),
            "abort" => Ok(Self::Abort),
            _ => Err(crate::error::RuntimeError::InvalidConfig(
                format!("invalid watchdog hang action '{text}'").into(),
            )),
        }
    }

    /// Configuration text of this action, as accepted by [`HangAction::parse`].
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fault => "fault",
            Self::Abort => "abort",
        }
    }
}

#[cfg(test)]
mod hang_action_tests {
    use super::HangAction;

    #[test]
    fn hang_action_accepts_fault_and_abort_ignoring_case_and_surrounding_space() {
        assert_eq!(
            HangAction::parse("fault").expect("fault"),
            HangAction::Fault
        );
        assert_eq!(
            HangAction::parse(" ABORT ").expect("abort"),
            HangAction::Abort
        );
        assert_eq!(HangAction::Abort.as_str(), "abort");
        assert_eq!(HangAction::default(), HangAction::Fault);
    }

    #[test]
    fn hang_action_rejects_other_text_with_the_watchdog_action_error_style() {
        let err = HangAction::parse(" exit ").expect_err("unknown hang action");
        assert!(
            err.to_string()
                .contains("invalid watchdog hang action ' exit '"),
            "{err}"
        );
    }
}
