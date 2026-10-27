impl DebugAdapter {
    #[must_use]
    pub fn new(session: impl DebugRuntime + 'static) -> Self {
        Self {
            session: Box::new(session),
            remote_session: None,
            remote_stop_poller: None,
            remote_io_poller: None,
            remote_breakpoints: Arc::new(Mutex::new(HashMap::new())),
            next_seq: Arc::new(AtomicU32::new(1)),
            coordinate: CoordinateConverter::new(true, true),
            expression_types: None,
            simulator: None,
            last_io_state: Arc::new(Mutex::new(None)),
            forced_io_addresses: Arc::new(Mutex::new(HashSet::new())),
            initialized: false,
            launch_state: LaunchState::default(),
            attach_breakpoints: Vec::new(),
            next_breakpoint_id: 1,
            pause_expected: Arc::new(AtomicBool::new(false)),
            stop_gate: StopGate::new(),
            dap_writer: None,
            dap_logger: None,
        }
    }

    pub fn session(&self) -> &dyn DebugRuntime {
        self.session.as_ref()
    }

    pub fn session_mut(&mut self) -> &mut dyn DebugRuntime {
        self.session.as_mut()
    }

    #[must_use]
    pub fn into_session(self) -> Box<dyn DebugRuntime> {
        self.session
    }

    /// Sets the breakpoints of one source on the connected runtime.
    #[must_use]
    pub fn set_breakpoints(&mut self, args: SetBreakpointsArguments) -> SetBreakpointsResponseBody {
        self.set_breakpoints_remote(args)
    }
}
