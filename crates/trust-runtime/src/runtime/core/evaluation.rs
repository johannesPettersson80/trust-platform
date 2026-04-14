impl Runtime {
    /// Evaluate a debug expression within the current runtime context.
    pub fn evaluate_expression(
        &mut self,
        expr: &Expr,
        frame_id: Option<FrameId>,
    ) -> Result<Value, error::RuntimeError> {
        let registry = &self.registry;
        let profile = &self.profile;
        let eval = |storage: &mut VariableStorage, instance_id: Option<InstanceId>| {
            crate::helper_eval::eval_storage_expr_with_stdlib(
                storage,
                registry,
                profile,
                instance_id,
                Some(&self.stdlib),
                expr,
            )
        };

        if let Some(frame_id) = frame_id {
            self.storage
                .with_frame(frame_id, |storage| {
                    let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
                    eval(storage, instance_id)
                })
                .ok_or(error::RuntimeError::InvalidFrame(frame_id.0))?
        } else {
            eval(&mut self.storage, None)
        }
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
            crate::helper_eval::read_storage_lvalue(storage, registry, profile, instance_id, target)
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
            crate::helper_eval::write_storage_lvalue(
                storage,
                registry,
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
