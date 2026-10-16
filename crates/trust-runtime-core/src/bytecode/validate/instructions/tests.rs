use super::*;
use alloc::vec;

#[test]
fn decoded_instruction_is_twenty_bytes_on_each_target() {
    assert_eq!(core::mem::size_of::<Instruction>(), 20);
}

// NOP, PUSH_CONST(7), JUMP to the end of the body.
const CODE: &[u8] = &[0x00, 0x10, 7, 0, 0, 0, 0x02, 0, 0, 0, 0];

#[test]
fn operand_callbacks_keep_wire_order_and_stop_before_later_instructions() {
    let expected = [(0, 0x00, 0), (1, 0x10, 7), (6, 0x02, 0)];
    let mut visits = Vec::new();
    let mut count = 4;
    let mut budget = ValidationBudget::new(ValidationLimits::default());
    let decoded = decode_instructions(CODE, &mut count, &mut budget, |item, _| {
        visits.push((item.pc, item.opcode, item.operand(0)));
        Ok(())
    })
    .unwrap();
    assert_eq!(count, 7);
    assert_eq!(visits, expected);
    assert_eq!(decoded.len(), 3);
    assert_eq!(decoded[2].jump_target().unwrap(), Some(CODE.len()));

    for stop in 0..expected.len() {
        let mut count = 4;
        let mut visits = Vec::new();
        let mut budget = ValidationBudget::new(ValidationLimits::default());
        let error = BytecodeError::InvalidPouId(271);
        let outcome = decode_instructions(CODE, &mut count, &mut budget, |item, _| {
            visits.push((item.pc, item.opcode, item.operand(0)));
            if visits.len() == stop + 1 {
                Err(error.clone())
            } else {
                Ok(())
            }
        });
        assert_eq!(outcome.unwrap_err(), error);
        assert_eq!(count, 5 + stop);
        assert_eq!(visits, expected[..=stop]);
    }
}

#[test]
fn framing_operand_and_jump_rejections_keep_their_precedence() {
    let mut budget = ValidationBudget::new(ValidationLimits::default());
    let mut count = 0;
    let mut visited = Vec::new();
    // An earlier operand rejection wins over a later truncated operand.
    let result = decode_instructions(&[0x00, 0x10], &mut count, &mut budget, |item, _| {
        visited.push(item.pc);
        Err(BytecodeError::InvalidPouId(19))
    });
    assert_eq!(result.unwrap_err(), BytecodeError::InvalidPouId(19));
    assert_eq!(count, 1);
    assert_eq!(visited, vec![0]);

    let mut budget = ValidationBudget::new(ValidationLimits::default());
    let mut count = 0;
    let mut visited = Vec::new();
    let result = decode_instructions(&[0x00, 0x10], &mut count, &mut budget, |item, _| {
        visited.push(item.pc);
        Ok(())
    });
    assert_eq!(result.unwrap_err(), BytecodeError::UnexpectedEof);
    assert_eq!(count, 2);
    assert_eq!(visited, vec![0]);

    let mut budget = ValidationBudget::new(ValidationLimits::default());
    let mut count = 0;
    let mut visited = Vec::new();
    // First jump is out of range, but all operand rules run before jump checks.
    let result = decode_instructions(
        &[0x02, 99, 0, 0, 0, 0x00],
        &mut count,
        &mut budget,
        |item, _| {
            visited.push(item.pc);
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), BytecodeError::InvalidJumpTarget(104));
    assert_eq!(count, 2);
    assert_eq!(visited, vec![0, 5]);
}

#[test]
fn decoder_never_runs_an_operand_callback_after_work_exhaustion() {
    let mut complete = ValidationBudget::new(ValidationLimits::default());
    let mut checkpoints = Vec::new();
    decode_instructions(CODE, &mut 0, &mut complete, |item, budget| {
        checkpoints.push((item.pc, budget.stats.work));
        Ok(())
    })
    .unwrap();
    let total = complete.stats.work;
    for limit in 0..=total {
        let mut budget = ValidationBudget::new(ValidationLimits {
            max_work: limit,
            ..ValidationLimits::default()
        });
        let mut visits = Vec::new();
        let result = decode_instructions(CODE, &mut 0, &mut budget, |item, budget| {
            visits.push((item.pc, budget.stats.work));
            Ok(())
        });
        assert_eq!(
            visits,
            checkpoints
                .iter()
                .copied()
                .filter(|(_, work)| *work <= limit)
                .collect::<Vec<_>>()
        );
        assert!(budget.stats.work <= limit);
        if limit < total {
            assert_eq!(
                result.unwrap_err(),
                BytecodeError::from(RejectionReason::ValidationWorkLimit)
            );
        } else {
            assert_eq!(result.unwrap().len(), 3);
        }
    }
}
