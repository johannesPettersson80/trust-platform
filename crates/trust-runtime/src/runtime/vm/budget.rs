use crate::error::RuntimeError;

use super::errors::VmTrap;

pub(super) fn consume_instruction_budget(
    budget: &mut usize,
    instructions: usize,
) -> Result<(), RuntimeError> {
    if instructions > *budget {
        return Err(VmTrap::BudgetExceeded.into_runtime_error());
    }
    *budget -= instructions;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_accepts_exact_remaining_work_and_rejects_the_next_instruction() {
        let mut budget = 3;
        consume_instruction_budget(&mut budget, 3).expect("exact remaining work must execute");
        assert_eq!(budget, 0);
        assert!(consume_instruction_budget(&mut budget, 1).is_err());
    }

    #[test]
    fn rejected_charge_does_not_underflow_the_remaining_budget() {
        let mut budget = 2;
        assert!(consume_instruction_budget(&mut budget, 3).is_err());
        assert_eq!(budget, 2);
    }
}
