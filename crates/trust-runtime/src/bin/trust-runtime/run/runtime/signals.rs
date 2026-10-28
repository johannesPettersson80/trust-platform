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

trait RuntimeShutdownTarget {
    fn request_shutdown(&self);
}

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

/// How often the console-stop thread looks at its flag.
#[cfg(not(unix))]
const CONSOLE_STOP_POLL: std::time::Duration = std::time::Duration::from_millis(100);

/// Ctrl+C (and a termination signal) on Windows request the ordinary bounded stop, as SIGINT and
/// SIGTERM do on Unix, so the runtime stops cleanly and removes its endpoint file (spec 11,
/// sections 6.8 and 6.8.0a). A Windows signal handler may only set a flag, so a thread waits for
/// the flag and then stops the runtime.
#[cfg(not(unix))]
fn install_runtime_signal_shutdown(
    target: trust_runtime::scheduler::ResourceControl<StdClock>,
) -> anyhow::Result<std::thread::JoinHandle<()>> {
    let requested = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    for signal in [
        signal_hook::consts::signal::SIGINT,
        signal_hook::consts::signal::SIGTERM,
    ] {
        signal_hook::flag::register(signal, std::sync::Arc::clone(&requested))
            .map_err(|err| anyhow::anyhow!("register console stop signal {signal}: {err}"))?;
    }
    std::thread::Builder::new()
        .name("trust-runtime-console-stop".to_string())
        .spawn(move || request_shutdown_when_flagged(&requested, &target, CONSOLE_STOP_POLL))
        .map_err(|err| anyhow::anyhow!("spawn console stop thread: {err}"))
}

/// Waits until `requested` is set, then requests the ordinary runtime stop.
#[cfg(any(test, not(unix)))]
fn request_shutdown_when_flagged<T: RuntimeShutdownTarget>(
    requested: &std::sync::atomic::AtomicBool,
    target: &T,
    poll: std::time::Duration,
) {
    while !requested.load(std::sync::atomic::Ordering::SeqCst) {
        std::thread::sleep(poll);
    }
    target.request_shutdown();
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
