#[derive(Debug, Subcommand)]
pub enum BenchAction {
    /// Benchmark cycle latency/throughput for a real project folder.
    #[command(name = "project")]
    Project {
        /// Project folder to compile and execute.
        #[arg(long = "project", alias = "bundle")]
        project: PathBuf,
        /// Number of measured cycles.
        #[arg(long, default_value_t = 2_000)]
        samples: usize,
        /// Warmup cycles executed before measurements begin.
        #[arg(long = "warmup-cycles", default_value_t = 200)]
        warmup_cycles: usize,
        /// Global values to capture at the end of the run.
        #[arg(long = "watch")]
        watch: Vec<String>,
        /// Enable the experimental tier-1 specialized executor during the run.
        #[arg(long = "tier1", default_value_t = false)]
        tier1: bool,
        /// Output format (`table`, `json`).
        #[arg(long, value_enum, default_value_t = BenchOutputFormat::Table)]
        output: BenchOutputFormat,
    },
    /// Benchmark runtime initialization and first-cycle startup cost.
    #[command(name = "init")]
    Init {
        /// Project folder to compile and initialize.
        #[arg(long = "project", alias = "bundle")]
        project: PathBuf,
        /// Number of measured initialization samples.
        #[arg(long, default_value_t = 1_000)]
        samples: usize,
        /// Warmup cycles executed after the first cycle before steady-state sampling.
        #[arg(long = "warmup-cycles", default_value_t = 0)]
        warmup_cycles: usize,
        /// Output format (`table`, `json`).
        #[arg(long, value_enum, default_value_t = BenchOutputFormat::Table)]
        output: BenchOutputFormat,
    },
    /// Benchmark T0 SHM one-way/round-trip latency and overrun counters.
    #[command(name = "t0-shm")]
    T0Shm {
        /// Number of benchmark samples.
        #[arg(long, default_value_t = 1_000)]
        samples: usize,
        /// Payload size in bytes.
        #[arg(long = "payload-bytes", default_value_t = 32)]
        payload_bytes: usize,
        /// Output format (`table`, `json`).
        #[arg(long, value_enum, default_value_t = BenchOutputFormat::Table)]
        output: BenchOutputFormat,
    },
    /// Benchmark synthetic mesh pub/sub + query/reply latency and jitter.
    #[command(name = "mesh-zenoh")]
    MeshZenoh {
        /// Number of benchmark samples.
        #[arg(long, default_value_t = 1_000)]
        samples: usize,
        /// Payload size in bytes.
        #[arg(long = "payload-bytes", default_value_t = 64)]
        payload_bytes: usize,
        /// Synthetic packet loss rate in `[0.0, 1.0]`.
        #[arg(long = "loss-rate", default_value_t = 0.0)]
        loss_rate: f64,
        /// Synthetic packet reorder rate in `[0.0, 1.0]`.
        #[arg(long = "reorder-rate", default_value_t = 0.0)]
        reorder_rate: f64,
        /// Output format (`table`, `json`).
        #[arg(long, value_enum, default_value_t = BenchOutputFormat::Table)]
        output: BenchOutputFormat,
    },
    /// Benchmark runtime-cloud dispatch preflight/dispatch/audit-correlation latency.
    #[command(name = "dispatch")]
    Dispatch {
        /// Number of benchmark samples.
        #[arg(long, default_value_t = 1_000)]
        samples: usize,
        /// Payload size in bytes.
        #[arg(long = "payload-bytes", default_value_t = 32)]
        payload_bytes: usize,
        /// Number of target runtimes per dispatch.
        #[arg(long, default_value_t = 3)]
        fanout: usize,
        /// Output format (`table`, `json`).
        #[arg(long, value_enum, default_value_t = BenchOutputFormat::Table)]
        output: BenchOutputFormat,
    },
}
