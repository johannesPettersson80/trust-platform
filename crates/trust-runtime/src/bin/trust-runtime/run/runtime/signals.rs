#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeShutdownSignal {
    Interrupt,
    Terminate,
}

#[cfg(unix)]
trait RuntimeSignalSource {
    fn recv_shutdown_signal(&mut self) -> std::io::Result<RuntimeShutdownSignal>;
}

#[cfg(unix)]
trait RuntimeShutdownTarget {
    fn request_shutdown(&self);
}

#[cfg(unix)]
impl<C> RuntimeShutdownTarget for trust_runtime::scheduler::ResourceControl<C>
where
    C: trust_runtime::scheduler::Clock + Clone,
{
    fn request_shutdown(&self) {
        self.stop();
    }
}

#[cfg(unix)]
fn request_shutdown_from_signal<S, T>(
    source: &mut S,
    target: &T,
) -> std::io::Result<RuntimeShutdownSignal>
where
    S: RuntimeSignalSource,
    T: RuntimeShutdownTarget,
{
    let signal = source.recv_shutdown_signal()?;
    target.request_shutdown();
    Ok(signal)
}

#[cfg(unix)]
fn install_runtime_signal_shutdown(
    target: trust_runtime::scheduler::ResourceControl<StdClock>,
) -> anyhow::Result<std::thread::JoinHandle<()>> {
    let mut source = OsSignalSource::new()?;
    std::thread::Builder::new()
        .name("trust-runtime-signal-shutdown".to_string())
        .spawn(move || {
            let _ = request_shutdown_from_signal(&mut source, &target);
        })
        .map_err(|err| anyhow::anyhow!("spawn signal shutdown thread: {err}"))
}

#[cfg(not(unix))]
fn install_runtime_signal_shutdown(
    _target: trust_runtime::scheduler::ResourceControl<StdClock>,
) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(unix)]
struct OsSignalSource {
    signals: signal_hook::iterator::Signals,
}

#[cfg(unix)]
impl OsSignalSource {
    fn new() -> std::io::Result<Self> {
        let signals = signal_hook::iterator::Signals::new([
            signal_hook::consts::signal::SIGINT,
            signal_hook::consts::signal::SIGTERM,
        ])?;
        Ok(Self { signals })
    }
}

#[cfg(unix)]
impl RuntimeSignalSource for OsSignalSource {
    fn recv_shutdown_signal(&mut self) -> std::io::Result<RuntimeShutdownSignal> {
        loop {
            match self.signals.forever().next() {
                Some(signal) if map_runtime_shutdown_signal(signal).is_some() => {
                    return Ok(map_runtime_shutdown_signal(signal).expect("mapped signal"));
                }
                Some(_) => continue,
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "signal iterator closed",
                    ));
                }
            }
        }
    }
}

#[cfg(unix)]
fn map_runtime_shutdown_signal(signal: i32) -> Option<RuntimeShutdownSignal> {
    match signal {
        signal_hook::consts::signal::SIGINT => Some(RuntimeShutdownSignal::Interrupt),
        signal_hook::consts::signal::SIGTERM => Some(RuntimeShutdownSignal::Terminate),
        _ => None,
    }
}
