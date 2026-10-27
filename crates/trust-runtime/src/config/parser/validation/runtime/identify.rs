/// `[runtime.identify]`: the output that `identify` blinks, or none when the section or its
/// `output` is absent (specification 11, "Identify").
fn parse_identify_section(
    section: Option<IdentifySection>,
) -> Result<Option<IdentifyOutputConfig>, RuntimeError> {
    section
        .and_then(|section| section.output)
        .map(|text| IdentifyOutputConfig::parse(&text))
        .transpose()
}
