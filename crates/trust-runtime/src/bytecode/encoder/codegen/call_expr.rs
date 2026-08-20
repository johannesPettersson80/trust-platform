#[derive(Clone, Copy)]
enum NativeTargetKind {
    Function,
    FunctionBlock,
    Method,
    Stdlib,
}

enum NativeReceiver<'a> {
    None,
    Expression(&'a crate::program_model::Expr),
    SelfValue,
}

struct NativeCallTarget<'a> {
    kind: NativeTargetKind,
    name: SmolStr,
    receiver: NativeReceiver<'a>,
}

impl NativeTargetKind {
    fn encoded(self) -> u32 {
        match self {
            Self::Function => crate::bytecode::NATIVE_CALL_KIND_FUNCTION,
            Self::FunctionBlock => crate::bytecode::NATIVE_CALL_KIND_FUNCTION_BLOCK,
            Self::Method => crate::bytecode::NATIVE_CALL_KIND_METHOD,
            Self::Stdlib => crate::bytecode::NATIVE_CALL_KIND_STDLIB,
        }
    }
}

impl NativeReceiver<'_> {
    fn count(&self) -> usize {
        usize::from(!matches!(self, Self::None))
    }
}

impl<'a> BytecodeEncoder<'a> {
    fn emit_call_expr(
        &mut self,
        ctx: &CodegenContext,
        target: &crate::program_model::Expr,
        args: &[crate::program_model::CallArg],
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        if let crate::program_model::Expr::Name(name) = target {
            if name.eq_ignore_ascii_case("REF") {
                return self.emit_ref_builtin_call(ctx, args, code);
            }
        }

        let target = self.resolve_native_call_target(ctx, target)?;
        let Some(en_index) = self.named_call_arg_index(args, "EN")? else {
            self.emit_native_call(ctx, &target, args, None, code)?;
            return Ok(true);
        };
        let crate::program_model::ArgValue::Expr(en) = &args[en_index].value else {
            return Err(BytecodeError::InvalidSection(
                "CALL_NATIVE EN argument must be an expression".into(),
            ));
        };

        if !self.emit_expr(ctx, en, code)? {
            return Err(BytecodeError::InvalidSection(
                "unsupported CALL_NATIVE EN argument expression".into(),
            ));
        }
        let disabled_jump = self.emit_jump_placeholder(code, 0x04);

        self.emit_native_call(ctx, &target, args, Some(en_index), code)?;
        let end_jump = self.emit_jump_placeholder(code, 0x02);

        let disabled_offset = code.len();
        self.patch_jump(code, disabled_jump, disabled_offset)?;
        if let Some(eno_index) = self.named_call_arg_index(args, "ENO")? {
            let crate::program_model::ArgValue::Target(eno_target) = &args[eno_index].value else {
                return Err(BytecodeError::InvalidSection(
                    "CALL_NATIVE ENO argument must be a writable target".into(),
                ));
            };
            if !self.emit_assign(
                ctx,
                eno_target,
                &crate::program_model::Expr::Literal(Value::Bool(false)),
                code,
            )? {
                return Err(BytecodeError::InvalidSection(
                    "unsupported CALL_NATIVE ENO target".into(),
                ));
            }
        }
        let default = self.disabled_call_result_default(&target)?;
        if !self.emit_const_value(&default, code)? {
            return Err(BytecodeError::InvalidSection(
                "unsupported CALL_NATIVE disabled result default".into(),
            ));
        }
        let end_offset = code.len();
        self.patch_jump(code, end_jump, end_offset)?;
        Ok(true)
    }

    fn resolve_native_call_target<'b>(
        &mut self,
        ctx: &CodegenContext,
        target: &'b crate::program_model::Expr,
    ) -> Result<NativeCallTarget<'b>, BytecodeError> {
        match target {
            crate::program_model::Expr::Field {
                target: receiver,
                field,
            } => Ok(NativeCallTarget {
                kind: NativeTargetKind::Method,
                name: field.clone(),
                receiver: NativeReceiver::Expression(receiver),
            }),
            crate::program_model::Expr::Name(name) => {
                let key = SmolStr::new(name.to_ascii_uppercase());
                if ctx.local_ref(name).is_some()
                    || ctx.self_field_name(name).is_some()
                    || self.resolve_name_ref(ctx, name)?.is_some()
                {
                    Ok(NativeCallTarget {
                        kind: NativeTargetKind::FunctionBlock,
                        name: name.clone(),
                        receiver: NativeReceiver::Expression(target),
                    })
                } else if let Some(function_name) = self.resolve_function_call_name(ctx, name) {
                    Ok(NativeCallTarget {
                        kind: NativeTargetKind::Function,
                        name: function_name,
                        receiver: NativeReceiver::None,
                    })
                } else if self.runtime.stdlib().get(name.as_str()).is_some()
                    || crate::stdlib::time::is_runtime_clock_name(key.as_str())
                    || crate::stdlib::time::is_split_name(key.as_str())
                    || crate::stdlib::conversions::is_conversion_name(key.as_str())
                {
                    Ok(NativeCallTarget {
                        kind: NativeTargetKind::Stdlib,
                        name: name.clone(),
                        receiver: NativeReceiver::None,
                    })
                } else {
                    Ok(NativeCallTarget {
                        kind: NativeTargetKind::Method,
                        name: name.clone(),
                        receiver: NativeReceiver::SelfValue,
                    })
                }
            }
            _ => Err(BytecodeError::InvalidSection(
                "unsupported CALL_NATIVE target expression".into(),
            )),
        }
    }

    fn emit_native_call(
        &mut self,
        ctx: &CodegenContext,
        target: &NativeCallTarget<'_>,
        args: &[crate::program_model::CallArg],
        evaluated_en: Option<usize>,
        code: &mut Vec<u8>,
    ) -> Result<(), BytecodeError> {
        match &target.receiver {
            NativeReceiver::None => {}
            NativeReceiver::Expression(receiver) => {
                if !self.emit_expr(ctx, receiver, code)? {
                    return Err(BytecodeError::InvalidSection(
                        "unsupported CALL_NATIVE receiver".into(),
                    ));
                }
            }
            NativeReceiver::SelfValue => code.push(0x23),
        }

        let mut arg_tokens = Vec::with_capacity(args.len());
        for (index, arg) in args.iter().enumerate() {
            let prefix = if evaluated_en == Some(index) {
                if !self.emit_const_value(&Value::Bool(true), code)? {
                    return Err(BytecodeError::InvalidSection(
                        "unsupported CALL_NATIVE enabled EN value".into(),
                    ));
                }
                "E"
            } else {
                match &arg.value {
                    crate::program_model::ArgValue::Expr(expr) => {
                        if !self.emit_expr(ctx, expr, code)? {
                            return Err(BytecodeError::InvalidSection(
                                "unsupported CALL_NATIVE argument expression".into(),
                            ));
                        }
                        "E"
                    }
                    crate::program_model::ArgValue::Target(target) => {
                        if let Some(reference) = self.resolve_lvalue_ref(ctx, target)? {
                            let ref_idx = self.ref_index_for(&reference)?;
                            code.push(0x22);
                            code.extend_from_slice(&ref_idx.to_le_bytes());
                        } else if !self.emit_dynamic_ref_for_lvalue(ctx, target, code)? {
                            return Err(BytecodeError::InvalidSection(
                                format!("unsupported CALL_NATIVE argument target: {target:?}")
                                    .into(),
                            ));
                        }
                        "T"
                    }
                }
            };
            arg_tokens.push(match &arg.name {
                Some(name) => SmolStr::new(format!("{prefix}:{}", name.as_str())),
                None => SmolStr::new(prefix),
            });
        }

        let symbol_idx = self.intern_native_call_symbol(&target.name, &arg_tokens);
        let total_arg_count = args.len().saturating_add(target.receiver.count());
        let arg_count = u32::try_from(total_arg_count)
            .map_err(|_| BytecodeError::InvalidSection("CALL_NATIVE arg_count overflow".into()))?;
        code.push(0x09);
        code.extend_from_slice(&target.kind.encoded().to_le_bytes());
        code.extend_from_slice(&symbol_idx.to_le_bytes());
        code.extend_from_slice(&arg_count.to_le_bytes());
        Ok(())
    }

    fn named_call_arg_index(
        &self,
        args: &[crate::program_model::CallArg],
        expected: &str,
    ) -> Result<Option<usize>, BytecodeError> {
        let mut matches = args.iter().enumerate().filter_map(|(index, arg)| {
            arg.name
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(expected))
                .then_some(index)
        });
        let first = matches.next();
        if matches.next().is_some() {
            return Err(BytecodeError::InvalidSection(
                format!("duplicate CALL_NATIVE {expected} argument").into(),
            ));
        }
        Ok(first)
    }

    fn disabled_call_result_default(
        &self,
        target: &NativeCallTarget<'_>,
    ) -> Result<Value, BytecodeError> {
        let return_type = match target.kind {
            NativeTargetKind::Function => {
                let key = SmolStr::new(target.name.to_ascii_uppercase());
                Some(
                    self.runtime
                        .functions()
                        .get(&key)
                        .ok_or_else(|| {
                            BytecodeError::InvalidSection(
                                format!("missing disabled-call function '{}'", target.name).into(),
                            )
                        })?
                        .return_type,
                )
            }
            NativeTargetKind::FunctionBlock | NativeTargetKind::Stdlib => None,
            NativeTargetKind::Method => self.unique_method_return_type(&target.name)?,
        };
        let Some(return_type) = return_type else {
            return Ok(Value::Null);
        };
        crate::harness::initializer::default_value_for_type_id(
            self.runtime.storage(),
            self.runtime.registry(),
            self.runtime.initializer_catalog(),
            &self.runtime.profile(),
            None,
            self.runtime.stdlib(),
            return_type,
        )
        .map_err(|error| BytecodeError::InvalidSection(error.to_string().into()))
    }

    fn unique_method_return_type(
        &self,
        method_name: &SmolStr,
    ) -> Result<Option<trust_hir::TypeId>, BytecodeError> {
        let mut resolved = None;
        for return_type in self
            .runtime
            .classes()
            .values()
            .flat_map(|owner| owner.methods.iter())
            .chain(
                self.runtime
                    .function_blocks()
                    .values()
                    .flat_map(|owner| owner.methods.iter()),
            )
            .filter(|method| method.name.eq_ignore_ascii_case(method_name.as_str()))
            .map(|method| method.return_type)
        {
            if let Some(existing) = resolved {
                if existing != return_type {
                    return Err(BytecodeError::InvalidSection(
                        format!(
                            "ambiguous disabled-call result type for method '{method_name}'"
                        )
                        .into(),
                    ));
                }
            } else {
                resolved = Some(return_type);
            }
        }
        Ok(resolved.flatten())
    }

    fn resolve_function_call_name(
        &self,
        ctx: &CodegenContext,
        name: &SmolStr,
    ) -> Option<SmolStr> {
        let key = SmolStr::new(name.to_ascii_uppercase());
        if let Some(function) = self.runtime.functions().get(&key) {
            return Some(function.name.clone());
        }
        if name.contains('.') {
            return None;
        }
        for namespace in &ctx.using {
            let qualified = SmolStr::new(format!("{namespace}.{name}"));
            let key = SmolStr::new(qualified.to_ascii_uppercase());
            if let Some(function) = self.runtime.functions().get(&key) {
                return Some(function.name.clone());
            }
        }
        None
    }
}
