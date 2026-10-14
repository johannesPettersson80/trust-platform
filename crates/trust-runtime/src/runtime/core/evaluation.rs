impl Runtime {
    /// Evaluate using a parser-owned type registry, without changing installed types.
    ///
    /// Debug lowering may define anonymous SIZEOF types. The caller retains this
    /// scratch registry for the expression's complete lifetime, including watches.
    pub fn evaluate_debug_expression(
        &self,
        expr: &Expr,
        registry: &TypeRegistry,
        frame_id: Option<FrameId>,
    ) -> Result<Value, error::RuntimeError> {
        let mut storage = self.storage().clone();
        let eval = |storage: &mut VariableStorage, instance_id: Option<InstanceId>| {
            crate::helper_eval::eval_storage_expr_with_abi(
                storage,
                registry,
                crate::value::StorageAbi::LegacyTemporal32,
                &self.profile,
                instance_id,
                Some(&self.stdlib),
                expr,
            )
        };
        if let Some(frame_id) = frame_id {
            storage
                .with_frame(frame_id, |storage| {
                    let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
                    eval(storage, instance_id)
                })
                .ok_or(error::RuntimeError::InvalidFrame(frame_id.0))?
        } else {
            eval(&mut storage, None)
        }
    }

    /// Evaluate a debug expression within the current runtime context.
    pub fn evaluate_expression(
        &mut self,
        expr: &Expr,
        frame_id: Option<FrameId>,
    ) -> Result<Value, error::RuntimeError> {
        self.evaluate_debug_expression(expr, &self.registry, frame_id)
    }

    /// Read a debugger-targeted lvalue within the current runtime context.
    pub fn read_lvalue(
        &mut self,
        target: &LValue,
        frame_id: Option<FrameId>,
    ) -> Result<Value, error::RuntimeError> {
        let registry = &self.registry;
        let profile = &self.profile;
        let read = |storage: &mut VariableStorage, instance_id: Option<InstanceId>| {
            crate::helper_eval::read_storage_lvalue_with_abi(
                storage,
                registry,
                crate::value::StorageAbi::LegacyTemporal32,
                profile,
                instance_id,
                target,
            )
        };

        if let Some(frame_id) = frame_id {
            self.storage
                .with_frame(frame_id, |storage| {
                    let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
                    read(storage, instance_id)
                })
                .ok_or(error::RuntimeError::InvalidFrame(frame_id.0))?
        } else {
            read(&mut self.storage, None)
        }
    }

    /// Write a debugger-targeted lvalue within the current runtime context.
    pub fn write_lvalue(
        &mut self,
        target: &LValue,
        value: Value,
        frame_id: Option<FrameId>,
    ) -> Result<(), error::RuntimeError> {
        let registry = &self.registry;
        let profile = &self.profile;
        let write = |storage: &mut VariableStorage, instance_id: Option<InstanceId>| {
            crate::helper_eval::write_storage_lvalue_with_abi(
                storage,
                registry,
                crate::value::StorageAbi::LegacyTemporal32,
                profile,
                instance_id,
                target,
                value.clone(),
            )
        };

        if let Some(frame_id) = frame_id {
            self.storage
                .with_frame(frame_id, |storage| {
                    let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
                    write(storage, instance_id)
                })
                .ok_or(error::RuntimeError::InvalidFrame(frame_id.0))?
        } else {
            write(&mut self.storage, None)
        }
    }
}
