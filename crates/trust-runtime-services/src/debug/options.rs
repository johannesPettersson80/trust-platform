//! The options of a `breakpoints.set` as the control protocol carries them (specification 11
//! §6.13.2): each line's hit condition, compiled condition and logpoint message, decoded.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::Deserialize;

use super::breakpoint::{BreakpointOptions, HitCondition};
use super::expression::{decode, Expression, EXPR_VERSION};
use super::logpoint::LogPart;

/// One line's options as a request carries them.
#[derive(Debug, Deserialize)]
pub struct WireOptions {
    #[serde(default)]
    condition: Option<WireExpression>,
    #[serde(default)]
    hit_condition: Option<WireHit>,
    #[serde(default)]
    log_message: Option<Vec<WirePart>>,
}

/// A compiled expression as a request carries it.
#[derive(Debug, Deserialize)]
struct WireExpression {
    expr_version: u32,
    expr: String,
}

/// A hit condition as a request carries it.
#[derive(Debug, Deserialize)]
struct WireHit {
    op: String,
    count: u64,
}

/// A part of a logpoint's message as a request carries it: a text or a compiled expression.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WirePart {
    Text(String),
    Expression(WireExpression),
}

/// Why a request's options are refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionsError {
    /// An expression of a version this runtime does not implement (`expr_version_unsupported`).
    Version(u32),
    /// An expression whose bytes do not decode (`expr_invalid`), with the detail.
    Invalid(String),
    /// Options that are not as the protocol defines them, with the detail.
    Malformed(String),
}

impl WireOptions {
    /// Whether the options carry an expression, which only a runtime that evaluates the compiled
    /// form takes.
    #[must_use]
    pub fn has_expressions(&self) -> bool {
        self.condition.is_some() || self.log_message.is_some()
    }

    /// The options, decoded.
    ///
    /// # Errors
    ///
    /// The [`OptionsError`] of a hit condition outside the protocol or of an expression this
    /// runtime does not take.
    pub fn decode(self) -> Result<BreakpointOptions, OptionsError> {
        let hit_condition = self.hit_condition.map(WireHit::decode).transpose()?;
        let condition = self.condition.map(WireExpression::decode).transpose()?;
        let log_message = self
            .log_message
            .map(|parts| {
                parts
                    .into_iter()
                    .map(|part| match part {
                        WirePart::Text(text) => Ok(LogPart::Text(text)),
                        WirePart::Expression(expression) => {
                            expression.decode().map(LogPart::Expression)
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        Ok(BreakpointOptions {
            hit_condition,
            condition,
            log_message,
        })
    }
}

impl WireHit {
    fn decode(self) -> Result<HitCondition, OptionsError> {
        if self.count == 0 {
            return Err(OptionsError::Malformed(String::from(
                "a hit condition's count is 0",
            )));
        }
        match self.op.as_str() {
            "=" => Ok(HitCondition::Equal(self.count)),
            ">=" => Ok(HitCondition::AtLeast(self.count)),
            ">" => Ok(HitCondition::GreaterThan(self.count)),
            other => Err(OptionsError::Malformed(format!(
                "unknown hit condition operator '{other}'"
            ))),
        }
    }
}

impl WireExpression {
    fn decode(self) -> Result<Expression, OptionsError> {
        if self.expr_version != EXPR_VERSION {
            return Err(OptionsError::Version(self.expr_version));
        }
        let bytes = STANDARD
            .decode(self.expr.as_bytes())
            .map_err(|_| OptionsError::Invalid(String::from("the expression is not base64")))?;
        decode(&bytes).map_err(|error| OptionsError::Invalid(String::from(error.detail())))
    }
}
