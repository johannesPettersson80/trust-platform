//! Capture explicit aggregate values before typed default/coercion callbacks run.

use super::*;
use crate::bytecode::BYTECODE_MAX_CONSTRUCTION_NODES;
use crate::program_model::{ArgValue, Expr};

// Hosted producers and supported portable targets have at least 32-bit usize.
const MAX_AGGREGATE_VALUES: usize = BYTECODE_MAX_CONSTRUCTION_NODES as usize;

impl BytecodeEncoder<'_> {
    pub(super) fn emit_initializer_aggregate(
        &mut self,
        ctx: &CodegenContext,
        expression: &Expr,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        let start = code.len();
        match expression {
            Expr::ArrayInitializer(elements) => {
                let mut expanded = Vec::new();
                for element in elements {
                    expand_element(element, &mut expanded, 0)?;
                }
                emit_operand(
                    code,
                    crate::bytecode::opcodes::ARRAY_NEW,
                    expanded.len() as u32,
                );
                for (index, element) in expanded.into_iter().enumerate() {
                    if !self.emit_expr(ctx, element, code)? {
                        code.truncate(start);
                        return Ok(false);
                    }
                    emit_operand(code, crate::bytecode::opcodes::ARRAY_SET, index as u32);
                }
            }
            Expr::StructInitializer(fields) => {
                if fields.len() > MAX_AGGREGATE_VALUES {
                    return Err(invalid());
                }
                let mut names = std::collections::HashSet::new();
                emit_operand(code, crate::bytecode::opcodes::STRUCT_NEW, 0);
                for (name, expression) in fields {
                    if !names.insert(name.to_ascii_uppercase()) {
                        return Err(BytecodeError::InvalidSection(
                            "duplicate initializer member".into(),
                        ));
                    }
                    if !self.emit_expr(ctx, expression, code)? {
                        code.truncate(start);
                        return Ok(false);
                    }
                    let name = self.strings.intern(name.clone());
                    emit_operand(code, crate::bytecode::opcodes::STRUCT_SET, name);
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

fn emit_operand(code: &mut Vec<u8>, opcode: u8, operand: u32) {
    code.push(opcode);
    code.extend_from_slice(&operand.to_le_bytes());
}

fn invalid() -> BytecodeError {
    BytecodeError::InvalidSection("array initializer expansion limit or invalid repeat".into())
}

fn expand_element<'a>(
    expression: &'a Expr,
    result: &mut Vec<&'a Expr>,
    depth: usize,
) -> Result<(), BytecodeError> {
    if depth > 64 {
        return Err(invalid());
    }
    if let Expr::Call { target, args } = expression {
        let count = match target.as_ref() {
            Expr::Literal(Value::SInt(v)) => Some(i128::from(*v)),
            Expr::Literal(Value::Int(v)) => Some(i128::from(*v)),
            Expr::Literal(Value::DInt(v)) => Some(i128::from(*v)),
            Expr::Literal(Value::LInt(v)) => Some(i128::from(*v)),
            Expr::Literal(Value::USInt(v)) => Some(i128::from(*v)),
            Expr::Literal(Value::UInt(v)) => Some(i128::from(*v)),
            Expr::Literal(Value::UDInt(v)) => Some(i128::from(*v)),
            Expr::Literal(Value::ULInt(v)) => Some(i128::from(*v)),
            _ => None,
        };
        if let Some(count) = count {
            let count = usize::try_from(count).map_err(|_| invalid())?;
            if count > MAX_AGGREGATE_VALUES || args.iter().any(|arg| arg.name.is_some()) {
                return Err(invalid());
            }
            let mut group = Vec::new();
            for arg in args {
                let ArgValue::Expr(expression) = &arg.value else {
                    return Err(invalid());
                };
                expand_element(expression, &mut group, depth + 1)?;
            }
            let added = count.checked_mul(group.len()).ok_or_else(invalid)?;
            if result
                .len()
                .checked_add(added)
                .is_none_or(|len| len > MAX_AGGREGATE_VALUES)
            {
                return Err(invalid());
            }
            result.try_reserve(added).map_err(|_| invalid())?;
            for _ in 0..count {
                result.extend_from_slice(&group);
            }
            return Ok(());
        }
    }
    if result.len() == MAX_AGGREGATE_VALUES {
        return Err(invalid());
    }
    result.try_reserve(1).map_err(|_| invalid())?;
    result.push(expression);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program_model::CallArg;

    fn repeat(count: u64, values: Vec<Expr>) -> Expr {
        Expr::Call {
            target: Box::new(Expr::Literal(Value::ULInt(count))),
            args: values
                .into_iter()
                .map(|value| CallArg {
                    name: None,
                    value: ArgValue::Expr(value),
                })
                .collect(),
        }
    }

    #[test]
    fn repeated_aggregate_expressions_keep_source_order_and_frequency() {
        let expression = repeat(
            2,
            vec![
                Expr::Name("first".into()),
                repeat(2, vec![Expr::Name("second".into())]),
            ],
        );
        let mut expanded = Vec::new();
        expand_element(&expression, &mut expanded, 0).unwrap();
        let names: Vec<_> = expanded
            .iter()
            .map(|expr| match expr {
                Expr::Name(name) => name.as_str(),
                _ => panic!("unexpected expanded value"),
            })
            .collect();
        assert_eq!(
            names,
            ["first", "second", "second", "first", "second", "second"]
        );
    }

    #[test]
    fn repeat_expansion_rejects_overflow_before_allocating_the_product() {
        let expression = repeat(
            BYTECODE_MAX_CONSTRUCTION_NODES as u64,
            vec![Expr::Literal(Value::Int(1)), Expr::Literal(Value::Int(2))],
        );
        let mut expanded = Vec::new();
        assert!(expand_element(&expression, &mut expanded, 0).is_err());
        assert!(expanded.is_empty());
        assert!(expand_element(&repeat(u64::MAX, vec![]), &mut expanded, 0).is_err());
    }
}
