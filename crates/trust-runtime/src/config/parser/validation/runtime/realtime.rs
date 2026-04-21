fn parse_realtime_section(section: Option<RealtimeSection>) -> Result<ParsedRealtime, RuntimeError> {
    let config = if let Some(section) = section {
        LinuxRtConfig {
            enabled: section.enabled.unwrap_or(false),
            require_preempt_rt_kernel: section.require_preempt_rt_kernel.unwrap_or(false),
            lock_memory: section.lock_memory.unwrap_or(false),
            scheduler: match section.scheduler.as_deref() {
                Some(value) => LinuxRtSchedulerPolicy::parse(value)?,
                None => LinuxRtSchedulerPolicy::Other,
            },
            priority: section.priority.unwrap_or(70),
            cpu_affinity: section.cpu_affinity.unwrap_or_default(),
            strict: section.strict.unwrap_or(false),
        }
    } else {
        LinuxRtConfig::default()
    };
    config.validate()?;
    Ok(ParsedRealtime { config })
}
