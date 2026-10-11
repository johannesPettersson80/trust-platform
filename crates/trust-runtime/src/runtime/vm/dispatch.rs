use super::super::core::Runtime;
use super::errors::VmTrap;
#[cfg(test)]
use super::frames::FrameStack;
use super::register_ir::{try_execute_pou_with_register_ir, RegisterExecutionOutcome};
#[cfg(test)]
use super::stack::OperandStack;
use super::VmModule;
use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::task::ProgramDef;
use crate::value::{Value, ValueRef};
use smol_str::SmolStr;
use std::cell::RefCell;
use std::time::Instant;
use trust_runtime_core::vm::hosted::dispatch::ExecutionBuffers;

const VM_EXECUTION_POOL_LIMIT: usize = 64;

thread_local! {
    static VM_EXECUTION_BUFFER_POOL: RefCell<Vec<ExecutionBuffers>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug)]
struct VmExecutionBuffers {
    buffers: Option<ExecutionBuffers>,
}

impl VmExecutionBuffers {
    fn acquire() -> Self {
        let buffers = VM_EXECUTION_BUFFER_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default();
        Self {
            buffers: Some(buffers),
        }
    }

    #[cfg(test)]
    fn stacks_mut(&mut self) -> (&mut OperandStack, &mut FrameStack) {
        let buffers = self.buffers.as_mut().expect("vm execution buffers missing");
        (&mut buffers.operand_stack, &mut buffers.frames)
    }
}

impl Drop for VmExecutionBuffers {
    fn drop(&mut self) {
        if let Some(mut buffers) = self.buffers.take() {
            buffers.clear();
            VM_EXECUTION_BUFFER_POOL.with(|pool| {
                let mut pool = pool.borrow_mut();
                if pool.len() < VM_EXECUTION_POOL_LIMIT {
                    pool.push(buffers);
                }
            });
        }
    }
}

pub(super) fn execute_program(
    runtime: &mut Runtime,
    program: &ProgramDef,
) -> Result<(), RuntimeError> {
    execute_program_by_name(runtime, &program.name)
}

pub(super) fn execute_program_by_name(
    runtime: &mut Runtime,
    program_name: &SmolStr,
) -> Result<(), RuntimeError> {
    let module = runtime.vm_module.clone().ok_or_else(|| {
        RuntimeError::InvalidConfig(
            "runtime.execution_backend='vm' requires loaded bytecode module".into(),
        )
    })?;

    let key = SmolStr::new(program_name.to_ascii_uppercase());
    let pou_id = module
        .program_ids()
        .get(&key)
        .copied()
        .ok_or_else(|| VmTrap::MissingProgram(program_name.clone()).into_runtime_error())?;

    let instance_id = match runtime.storage.get_global(program_name.as_ref()) {
        Some(Value::Instance(id)) => Some(*id),
        _ => None,
    };

    execute_pou(runtime, module.as_ref(), pou_id, instance_id)
}

pub(super) fn execute_function_block_ref(
    runtime: &mut Runtime,
    reference: &ValueRef,
) -> Result<(), RuntimeError> {
    let module = runtime.vm_module.clone().ok_or_else(|| {
        RuntimeError::InvalidConfig(
            "runtime.execution_backend='vm' requires loaded bytecode module".into(),
        )
    })?;

    let instance_id = match runtime.storage.read_by_ref_ref(reference) {
        Some(Value::Instance(id)) => *id,
        Some(_) => return Err(RuntimeError::TypeMismatch),
        None => return Err(RuntimeError::NullReference),
    };

    let instance = runtime
        .storage
        .get_instance(instance_id)
        .ok_or(RuntimeError::NullReference)?;
    let key = SmolStr::new(instance.type_name.to_ascii_uppercase());
    let pou_id = module
        .function_block_ids()
        .get(&key)
        .copied()
        .ok_or_else(|| {
            VmTrap::MissingFunctionBlock(instance.type_name.clone()).into_runtime_error()
        })?;

    execute_pou(runtime, module.as_ref(), pou_id, Some(instance_id))
}

fn execute_pou(
    runtime: &mut Runtime,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
) -> Result<(), RuntimeError> {
    let edge_transaction =
        super::edge::EdgeInputTransaction::begin(runtime, module, pou_id, entry_instance)?;
    let result = (|| -> Result<(), RuntimeError> {
        match try_execute_pou_with_register_ir(runtime, module, pou_id, entry_instance)? {
            RegisterExecutionOutcome::Executed => Ok(()),
            RegisterExecutionOutcome::FallbackToStack => {
                execute_pou_stack(runtime, module, pou_id, entry_instance)
            }
        }
    })();
    if let Some(edge_transaction) = edge_transaction {
        edge_transaction.restore(runtime);
    }
    result
}

fn execute_pou_stack(
    runtime: &mut Runtime,
    module: &VmModule,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
) -> Result<(), RuntimeError> {
    let _ = execute_pou_stack_with_locals(
        runtime,
        module,
        pou_id,
        entry_instance,
        None,
        false,
        0,
        trust_runtime_core::vm::hosted::budget::ExecutionEntry::Root,
    )?;
    Ok(())
}

pub(super) use trust_runtime_core::vm::hosted::dispatch::execute_pou_stack_with_locals;
#[cfg(test)]
use trust_runtime_core::vm::hosted::dispatch::{
    decode_partial_access, partial_access_error_to_runtime,
};

pub(super) fn take_shared_buffers() -> ExecutionBuffers {
    VmExecutionBuffers::acquire()
        .buffers
        .take()
        .unwrap_or_default()
}
pub(super) fn recycle_shared_buffers(buffers: ExecutionBuffers) {
    drop(VmExecutionBuffers {
        buffers: Some(buffers),
    });
}

pub(super) fn deadline_exceeded(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|deadline| Instant::now() >= deadline)
}

#[cfg(test)]
mod tests {
    use crate::value::{PartialAccess, PartialAccessError};
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn stack_deadline_stride_uses_shared_work_not_a_local_instruction_counter() {
        let budget = trust_runtime_core::vm::hosted::budget::ExecutionBudget::new(64);
        for _ in 0..31 {
            assert!(!budget.charge(1).unwrap());
        }
        assert!(budget.charge(1).unwrap());
        assert!(budget.charge(32).unwrap());
        assert_eq!(budget.remaining(), 0);
    }

    #[test]
    fn stack_deadline_distinguishes_missing_past_and_future_deadlines() {
        assert!(!deadline_exceeded(None));
        assert!(deadline_exceeded(Some(
            Instant::now() - Duration::from_millis(1)
        )));
        assert!(!deadline_exceeded(Some(
            Instant::now() + Duration::from_secs(30)
        )));
    }

    #[test]
    fn stack_partial_access_decoder_preserves_kind_and_rejects_reserved_bits() {
        assert_eq!(decode_partial_access(0x000), Ok(PartialAccess::Bit(0)));
        assert_eq!(decode_partial_access(0x0ff), Ok(PartialAccess::Bit(255)));
        assert_eq!(decode_partial_access(0x101), Ok(PartialAccess::Byte(1)));
        assert_eq!(decode_partial_access(0x202), Ok(PartialAccess::Word(2)));
        assert_eq!(decode_partial_access(0x303), Ok(PartialAccess::DWord(3)));
        assert_eq!(
            decode_partial_access(0x400),
            Err(RuntimeError::TypeMismatch)
        );

        assert_eq!(
            partial_access_error_to_runtime(PartialAccessError::IndexOutOfBounds {
                index: 32,
                lower: 0,
                upper: 31,
            }),
            RuntimeError::IndexOutOfBounds {
                index: 32,
                lower: 0,
                upper: 31,
            }
        );
        assert_eq!(
            partial_access_error_to_runtime(PartialAccessError::TypeMismatch),
            RuntimeError::TypeMismatch
        );
    }

    #[test]
    fn stack_execution_buffers_return_clean_and_respect_pool_limit() {
        clear_execution_pools();
        {
            let mut buffers = VmExecutionBuffers::acquire();
            let (operands, frames) = buffers.stacks_mut();
            operands.push(Value::DInt(7)).unwrap();
            frames.push(frame()).unwrap();
        }
        assert_eq!(execution_pool_lengths(), (1, 1));

        {
            let mut buffers = VmExecutionBuffers::acquire();
            let (operands, frames) = buffers.stacks_mut();
            assert!(matches!(operands.pop(), Err(VmTrap::StackUnderflow)));
            assert!(frames.is_empty());
        }

        clear_execution_pools();
        let buffers = (0..=VM_EXECUTION_POOL_LIMIT)
            .map(|_| VmExecutionBuffers::acquire())
            .collect::<Vec<_>>();
        drop(buffers);
        assert_eq!(
            execution_pool_lengths(),
            (VM_EXECUTION_POOL_LIMIT, VM_EXECUTION_POOL_LIMIT)
        );
        clear_execution_pools();
    }

    fn clear_execution_pools() {
        VM_EXECUTION_BUFFER_POOL.with(|pool| pool.borrow_mut().clear());
    }

    fn execution_pool_lengths() -> (usize, usize) {
        let pairs = VM_EXECUTION_BUFFER_POOL.with(|pool| pool.borrow().len());
        (pairs, pairs)
    }

    fn frame() -> super::super::frames::VmFrame {
        super::super::frames::VmFrame {
            parameter_values_present: Vec::new(),
            activation: None,
            pou_id: Some(1),
            return_pc: 0,
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            locals: Vec::new(),
            runtime_instance: None,
            instance_owner: None,
        }
    }
}
