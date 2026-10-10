use super::ConversionType;

/// Recognized conversion operation and explicit or inferred source type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionSpec {
    /// Value conversion with standard rounding policy.
    Convert {
        /// Optional explicit source category; otherwise inferred from the value.
        src: Option<ConversionType>,
        /// Required destination category.
        dst: ConversionType,
    },
    /// Truncating real-to-integer conversion.
    Trunc {
        /// Optional explicit source category; otherwise inferred from the value.
        src: Option<ConversionType>,
        /// Required destination category.
        dst: ConversionType,
    },
    /// Encode an integer as packed BCD.
    ToBcd {
        /// Optional explicit source category; otherwise inferred from the value.
        src: Option<ConversionType>,
        /// Required destination category.
        dst: ConversionType,
    },
    /// Decode packed BCD to an integer.
    BcdTo {
        /// Optional explicit source category; otherwise inferred from the value.
        src: Option<ConversionType>,
        /// Required destination category.
        dst: ConversionType,
    },
}

pub(super) fn parse_conversion_spec(name: &str) -> Option<ConversionSpec> {
    let upper = name.to_ascii_uppercase();

    if upper == "TRUNC" {
        return Some(ConversionSpec::Trunc {
            src: None,
            dst: ConversionType::DInt,
        });
    }

    if let Some(dst_name) = upper.strip_prefix("TRUNC_") {
        let dst = ConversionType::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::Trunc { src: None, dst });
    }

    if let Some((src_name, dst_name)) = upper.split_once("_TRUNC_") {
        let src = ConversionType::from_builtin_name(src_name)?;
        let dst = ConversionType::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::Trunc {
            src: Some(src),
            dst,
        });
    }

    if let Some(dst_name) = upper.strip_prefix("TO_BCD_") {
        let dst = ConversionType::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::ToBcd { src: None, dst });
    }

    if let Some((src_name, dst_name)) = upper.split_once("_TO_BCD_") {
        let src = ConversionType::from_builtin_name(src_name)?;
        let dst = ConversionType::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::ToBcd {
            src: Some(src),
            dst,
        });
    }

    if let Some(dst_name) = upper.strip_prefix("BCD_TO_") {
        let dst = ConversionType::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::BcdTo { src: None, dst });
    }

    if let Some((src_name, dst_name)) = upper.split_once("_BCD_TO_") {
        let src = ConversionType::from_builtin_name(src_name)?;
        let dst = ConversionType::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::BcdTo {
            src: Some(src),
            dst,
        });
    }

    if let Some(dst_name) = upper.strip_prefix("TO_") {
        let dst = ConversionType::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::Convert { src: None, dst });
    }

    if let Some((src_name, dst_name)) = upper.split_once("_TO_") {
        let src = ConversionType::from_builtin_name(src_name)?;
        let dst = ConversionType::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::Convert {
            src: Some(src),
            dst,
        });
    }

    None
}
