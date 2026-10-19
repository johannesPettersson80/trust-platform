#[derive(Debug, Clone)]
pub struct EthercatConfig {
    pub adapter: SmolStr,
    pub timeout: StdDuration,
    pub cycle_warn: StdDuration,
    pub on_error: IoDriverErrorPolicy,
    pub modules: Vec<EthercatModuleConfig>,
    pub expected_input_bytes: usize,
    pub expected_output_bytes: usize,
    pub mock_inputs: Vec<Vec<u8>>,
    pub mock_latency: StdDuration,
    pub mock_fail_read: bool,
    pub mock_fail_write: bool,
}

#[derive(Debug, Clone)]
pub struct EthercatModuleConfig {
    pub model: SmolStr,
    pub slot: u16,
    pub channels: u16,
}

#[derive(Debug, Deserialize)]
struct EthercatToml {
    adapter: Option<String>,
    timeout_ms: Option<u64>,
    cycle_warn_ms: Option<u64>,
    on_error: Option<String>,
    modules: Option<Vec<EthercatModuleToml>>,
    mock_inputs: Option<Vec<String>>,
    mock_latency_ms: Option<u64>,
    mock_fail_read: Option<bool>,
    mock_fail_write: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct EthercatModuleToml {
    model: String,
    slot: Option<u16>,
    channels: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EthercatModuleKind {
    Coupler,
    DigitalInput,
    DigitalOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EthercatDiscovery {
    subdevices: Vec<EthercatSubDevice>,
    input_bytes: usize,
    output_bytes: usize,
}

/// One SubDevice that an identification found (specification 11, "Identity" and "Sources"): its
/// position and addresses on the bus, its identity, and its mapped process data lengths.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EthercatSubDevice {
    slot: u16,
    configured_address: u16,
    alias_address: u16,
    name: SmolStr,
    channels: u16,
    vendor_id: u32,
    product_id: u32,
    revision: u32,
    serial: u32,
    input_bytes: usize,
    output_bytes: usize,
}

trait EthercatBus: Send {
    fn discover(&mut self, config: &EthercatConfig) -> Result<EthercatDiscovery, RuntimeError>;
    fn read_inputs(&mut self, bytes: usize) -> Result<Vec<u8>, RuntimeError>;
    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError>;
}
