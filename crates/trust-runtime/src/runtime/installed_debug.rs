//! Development-only evaluation stays detached from installed execution state.

use super::Runtime;
use crate::{
    debug::DebugSnapshot,
    error::RuntimeError,
    memory::FrameId,
    program_model::{Expr, LValue},
    value::Value,
};
use trust_hir::types::TypeRegistry;

impl Runtime {
    fn inspection_snapshot(&self, frame: Option<FrameId>) -> Result<DebugSnapshot, RuntimeError> {
        if let Some(frame) = frame {
            let snapshot = self
                .debug_control()
                .filter(|debug| debug.is_paused())
                .and_then(|debug| debug.snapshot())
                .ok_or(RuntimeError::InvalidFrame(frame.0))?;
            if !snapshot
                .storage
                .frames()
                .iter()
                .any(|candidate| candidate.id == frame)
            {
                return Err(RuntimeError::InvalidFrame(frame.0));
            }
            return Ok(snapshot);
        }
        Ok(DebugSnapshot {
            storage_abi: self.prepared().storage_abi(),
            storage: self.storage().clone(),
            now: self.current_time(),
        })
    }

    /// Evaluate with the parser's expression-lifetime registry on a detached
    /// snapshot. Anonymous parser types never enter executable TYPE_TABLE.
    pub fn evaluate_debug_expression(
        &self,
        expr: &Expr,
        registry: &TypeRegistry,
        frame: Option<FrameId>,
    ) -> Result<Value, RuntimeError> {
        self.inspection_snapshot(frame)?.evaluate_expression(
            expr,
            registry,
            Some(self.stdlib()),
            &self.profile(),
            frame,
        )
    }

    /// Resolve index/dereference expressions before submitting a checked value
    /// update. The resolved destination carries no parser-only type identities.
    pub fn resolve_debug_lvalue(
        &self,
        target: LValue,
        registry: &TypeRegistry,
        frame: Option<FrameId>,
    ) -> Result<LValue, RuntimeError> {
        target
            .resolve_expressions(&mut |expr| self.evaluate_debug_expression(expr, registry, frame))
    }

    /// Preserve the existing expression inspection surface on immutable metadata.
    pub fn evaluate_expression(
        &mut self,
        expr: &Expr,
        frame: Option<FrameId>,
    ) -> Result<Value, RuntimeError> {
        self.evaluate_debug_expression(expr, self.registry(), frame)
    }

    /// Read a debugger lvalue from the same detached expression context.
    pub fn read_lvalue(
        &mut self,
        target: &LValue,
        frame: Option<FrameId>,
    ) -> Result<Value, RuntimeError> {
        self.inspection_snapshot(frame)?.read_lvalue(
            target,
            self.registry(),
            &self.profile(),
            frame,
        )
    }

    /// Commit an owner-local value update through reference policy. Paused caller
    /// frames belong to detached snapshots and cannot be mutated through this
    /// controller; their existing queued path is rechecked at the next cycle.
    pub fn write_lvalue(
        &mut self,
        target: &LValue,
        value: Value,
        frame: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        if frame.is_some() {
            return Err(RuntimeError::ReferenceLifetime);
        }
        self.write_lvalue_in_instance(target, value, None)
    }

    pub(crate) fn write_lvalue_in_instance(
        &mut self,
        target: &LValue,
        value: Value,
        instance: Option<crate::memory::InstanceId>,
    ) -> Result<(), RuntimeError> {
        let (target, partial) = match target {
            LValue::Field { target, field }
                if crate::value::parse_partial_access(field).is_some() =>
            {
                (&**target, crate::value::parse_partial_access(field))
            }
            other => (other, None),
        };
        let reference = crate::helper_eval::eval_storage_expr_with_abi(
            self.storage(),
            self.registry(),
            self.prepared().storage_abi(),
            &self.profile(),
            instance,
            Some(self.stdlib()),
            &Expr::Ref(target.clone()),
        )?;
        let Value::Reference(Some(reference)) = reference else {
            return Err(RuntimeError::NullReference);
        };
        let value = match partial {
            Some(partial) => crate::value::write_partial_access(
                self.state.read_reference(&reference)?,
                partial,
                value,
            )
            .map_err(RuntimeError::from)?,
            None => value,
        };
        self.state.write_reference(reference.as_view(), value)
    }

    /// Resolve source namespaces from a currently paused detached frame.
    pub fn using_for_frame(&self, frame: FrameId) -> Option<Vec<smol_str::SmolStr>> {
        self.inspection_snapshot(Some(frame))
            .ok()
            .and_then(|snapshot| self.metadata.using_for_frame(&snapshot.storage, frame))
    }
    /// Inspect installed source statement boundaries, when source text is available.
    pub fn statement_locations(&self, file_id: u32) -> Option<&[crate::debug::SourceLocation]> {
        self.metadata.statement_locations(file_id)
    }
    /// Resolve a source breakpoint against immutable installation metadata.
    pub fn resolve_breakpoint_location(
        &self,
        source: &str,
        file_id: u32,
        line: u32,
        column: u32,
    ) -> Option<crate::debug::SourceLocation> {
        self.metadata
            .resolve_breakpoint_location(source, file_id, line, column)
    }
    /// Return the resolved source breakpoint and presentation coordinates.
    pub fn resolve_breakpoint_position(
        &self,
        source: &str,
        file_id: u32,
        line: u32,
        column: u32,
    ) -> Option<(crate::debug::SourceLocation, u32, u32)> {
        self.metadata
            .resolve_breakpoint_position(source, file_id, line, column)
    }
}
