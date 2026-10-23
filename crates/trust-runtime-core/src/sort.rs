//! In-place heapsort control flow shared by bounded admission and preparation.
//!
//! The driver erases record and comparator types, so each record shape retains only
//! its small operation adapter. It needs no allocation and preserves the validator's
//! existing comparison, swap and charge order (including equal-key permutations).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operation {
    Charge,
    Less(usize, usize),
    Swap(usize, usize),
}

#[inline(never)]
pub(crate) fn heap_sort<E>(
    len: usize,
    operate: &mut dyn FnMut(Operation) -> Result<bool, E>,
) -> Result<(), E> {
    let mut failure = None;
    let _ = heap_sort_driver(len, &mut |operation| match operate(operation) {
        Ok(value) => Some(value),
        Err(error) => {
            failure = Some(error);
            None
        }
    });
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

// Error payloads remain in the typed facade; the complete sorting loop is shared
// by decoder, validator, preparation and execution budget error types.
#[inline(never)]
fn heap_sort_driver(len: usize, operate: &mut dyn FnMut(Operation) -> Option<bool>) -> Option<()> {
    fn sift(
        len: usize,
        mut root: usize,
        operate: &mut dyn FnMut(Operation) -> Option<bool>,
    ) -> Option<()> {
        while root < len / 2 {
            let mut child = root * 2 + 1;
            operate(Operation::Charge)?;
            if child + 1 < len && operate(Operation::Less(child, child + 1))? {
                child += 1;
            }
            operate(Operation::Charge)?;
            if !operate(Operation::Less(root, child))? {
                break;
            }
            operate(Operation::Charge)?;
            operate(Operation::Swap(root, child))?;
            root = child;
        }
        Some(())
    }
    for root in (0..len / 2).rev() {
        sift(len, root, operate)?;
    }
    for end in (1..len).rev() {
        operate(Operation::Charge)?;
        operate(Operation::Swap(0, end))?;
        sift(end, 0, operate)?;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::String, vec, vec::Vec};

    // Frozen pre-refactor driver: the oracle includes equal-key permutation and
    // every charge/comparison/swap callback, not just the final sorted values.
    fn original_heap_sort<E>(
        len: usize,
        operate: &mut dyn FnMut(Operation) -> Result<bool, E>,
    ) -> Result<(), E> {
        fn sift<E>(
            len: usize,
            mut root: usize,
            operate: &mut dyn FnMut(Operation) -> Result<bool, E>,
        ) -> Result<(), E> {
            while root < len / 2 {
                let mut child = root * 2 + 1;
                operate(Operation::Charge)?;
                if child + 1 < len && operate(Operation::Less(child, child + 1))? {
                    child += 1;
                }
                operate(Operation::Charge)?;
                if !operate(Operation::Less(root, child))? {
                    break;
                }
                operate(Operation::Charge)?;
                operate(Operation::Swap(root, child))?;
                root = child;
            }
            Ok(())
        }
        for root in (0..len / 2).rev() {
            sift(len, root, operate)?;
        }
        for end in (1..len).rev() {
            operate(Operation::Charge)?;
            operate(Operation::Swap(0, end))?;
            sift(end, 0, operate)?;
        }
        Ok(())
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Failure {
        step: usize,
        operation: Operation,
        context: String,
    }
    #[derive(Debug, PartialEq, Eq)]
    struct Outcome {
        values: Vec<(u8, usize)>,
        trace: Vec<Operation>,
        result: Result<(), Failure>,
    }

    fn exercise(keys: &[u8], failure_at: Option<usize>, original: bool) -> Outcome {
        let mut values: Vec<_> = keys
            .iter()
            .copied()
            .enumerate()
            .map(|(index, key)| (key, index))
            .collect();
        let mut trace = Vec::new();
        let len = values.len();
        let mut operate = |operation| {
            let step = trace.len();
            trace.push(operation);
            if failure_at == Some(step) {
                return Err(Failure {
                    step,
                    operation,
                    context: "original typed diagnostic payload".into(),
                });
            }
            match operation {
                Operation::Charge => Ok(false),
                Operation::Less(left, right) => Ok(values[left].0 < values[right].0),
                Operation::Swap(left, right) => {
                    values.swap(left, right);
                    Ok(false)
                }
            }
        };
        let result = if original {
            original_heap_sort(len, &mut operate)
        } else {
            heap_sort(len, &mut operate)
        };
        Outcome {
            values,
            trace,
            result,
        }
    }

    #[test]
    fn shared_driver_preserves_error_payload_trace_and_partial_permutation_at_every_step() {
        for keys in [
            vec![],
            vec![1],
            vec![2, 1],
            vec![2, 1, 2, 0, 1],
            vec![7, 6, 5, 4, 3, 2, 1, 0],
        ] {
            let expected = exercise(&keys, None, true);
            assert!(expected.result.is_ok());
            assert!(expected
                .values
                .windows(2)
                .all(|pair| pair[0].0 <= pair[1].0));
            assert_eq!(exercise(&keys, None, false), expected);
            for step in 0..expected.trace.len() {
                let stopped = exercise(&keys, Some(step), true);
                assert_eq!(
                    stopped.trace.len(),
                    step + 1,
                    "no callback after the original failure"
                );
                assert!(stopped.result.is_err());
                assert_eq!(
                    exercise(&keys, Some(step), false),
                    stopped,
                    "failure at callback {step}"
                );
            }
        }
    }
}
