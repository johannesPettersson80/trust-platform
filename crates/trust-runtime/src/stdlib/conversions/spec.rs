use trust_hir::TypeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversionSpec {
    Convert { src: Option<TypeId>, dst: TypeId },
    Trunc { src: Option<TypeId>, dst: TypeId },
    ToBcd { src: Option<TypeId>, dst: TypeId },
    BcdTo { src: Option<TypeId>, dst: TypeId },
}

pub(super) fn parse_conversion_spec(name: &str) -> Option<ConversionSpec> {
    let upper = name.to_ascii_uppercase();

    if upper == "TRUNC" {
        return Some(ConversionSpec::Trunc {
            src: None,
            dst: TypeId::DINT,
        });
    }

    if let Some(dst_name) = upper.strip_prefix("TRUNC_") {
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::Trunc { src: None, dst });
    }

    if let Some((src_name, dst_name)) = upper.split_once("_TRUNC_") {
        let src = TypeId::from_builtin_name(src_name)?;
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::Trunc {
            src: Some(src),
            dst,
        });
    }

    if let Some(dst_name) = upper.strip_prefix("TO_BCD_") {
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::ToBcd { src: None, dst });
    }

    if let Some((src_name, dst_name)) = upper.split_once("_TO_BCD_") {
        let src = TypeId::from_builtin_name(src_name)?;
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::ToBcd {
            src: Some(src),
            dst,
        });
    }

    if let Some(dst_name) = upper.strip_prefix("BCD_TO_") {
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::BcdTo { src: None, dst });
    }

    if let Some((src_name, dst_name)) = upper.split_once("_BCD_TO_") {
        let src = TypeId::from_builtin_name(src_name)?;
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::BcdTo {
            src: Some(src),
            dst,
        });
    }

    if let Some(dst_name) = upper.strip_prefix("TO_") {
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::Convert { src: None, dst });
    }

    if let Some((src_name, dst_name)) = upper.split_once("_TO_") {
        let src = TypeId::from_builtin_name(src_name)?;
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(ConversionSpec::Convert {
            src: Some(src),
            dst,
        });
    }

    None
}
